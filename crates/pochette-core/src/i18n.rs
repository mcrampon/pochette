//! The languages Pochette speaks, and finding its texts in the one the user reads, with
//! [Fluent](https://projectfluent.org). Each app has its own texts; the core has those both
//! show, as [`crate::format`] words them.
//!
//! A translation is a `.ftl` file named after its language, as `fr-CA.ftl`, built into the
//! app. US English is the source, every other language falls back on it, message by message.

use std::sync::{PoisonError, RwLock};

pub use fluent_bundle::FluentArgs;
use fluent_bundle::{FluentResource, concurrent::FluentBundle};
use unic_langid::LanguageIdentifier;

/// The language the texts are written in, which the others fall back on.
pub const SOURCE: &str = "en-US";

/// The languages the user can pick, by BCP 47 tag, with their names in themselves.
pub const LANGUAGES: [(&str, &str); 10] = [
    ("de", "Deutsch"),
    ("en-GB", "English (UK)"),
    ("en-US", "English (US)"),
    ("es", "Español"),
    ("fr", "Français"),
    ("fr-CA", "Français (Canada)"),
    ("it", "Italiano"),
    ("pl", "Polski"),
    ("pt-BR", "Português (Brasil)"),
    ("zh-Hans", "中文（简体）"),
];

/// Texts in one language and those it falls back on.
pub struct Catalog {
    /// The best first: a message missing from one is looked up in the next.
    bundles: Vec<FluentBundle<FluentResource>>,
}

impl Catalog {
    /// The texts of `translations`, by language tag, in the first of `languages` they have,
    /// then the next and so on, then in [`SOURCE`], which `translations` must have.
    ///
    /// `languages` are BCP 47 tags or POSIX locale names, as `fr_CA.UTF-8`, best first.
    #[must_use]
    pub fn new(translations: &[(&str, &str)], languages: &[impl AsRef<str>]) -> Self {
        let available: Vec<&str> = translations.iter().map(|(tag, _)| *tag).collect();
        let bundles = negotiate(languages, &available)
            .into_iter()
            .filter_map(|tag| {
                let (tag, source) = translations.iter().find(|(t, _)| *t == tag)?;
                bundle(tag, source)
            })
            .collect();
        Self { bundles }
    }

    /// The message `id`, with `args` in its placeholders, or `id` itself when no language
    /// has it.
    #[must_use]
    pub fn text(&self, id: &str, args: Option<&FluentArgs>) -> String {
        for bundle in &self.bundles {
            let Some(pattern) = bundle.get_message(id).and_then(|message| message.value()) else {
                continue;
            };
            let mut errors = Vec::new();
            let text = bundle.format_pattern(pattern, args, &mut errors);
            if !errors.is_empty() {
                tracing::warn!("the text {id} has errors: {errors:?}");
            }
            return text.into_owned();
        }
        tracing::warn!("no text {id}");
        id.to_owned()
    }

    /// Whether some language has the message `id`.
    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.bundles.iter().any(|bundle| bundle.has_message(id))
    }
}

/// A bundle of the messages of `source`, a `.ftl` file in the language `tag`.
fn bundle(tag: &str, source: &str) -> Option<FluentBundle<FluentResource>> {
    let language: LanguageIdentifier = tag.parse().ok()?;
    let resource =
        FluentResource::try_new(source.to_owned()).unwrap_or_else(|(resource, errors)| {
            tracing::warn!("the {tag} texts have errors: {errors:?}");
            resource
        });
    let mut bundle = FluentBundle::new_concurrent(vec![language]);
    // No marks around the values put in: they'd show in some places, as MPRIS titles, and
    // none of the languages is written right to left.
    bundle.set_use_isolating(false);
    if let Err(errors) = bundle.add_resource(resource) {
        tracing::warn!("the {tag} texts have errors: {errors:?}");
    }
    Some(bundle)
}

/// The tags of `available` to look texts up in, best first, for the user's `languages`,
/// ending with [`SOURCE`].
fn negotiate<'a>(languages: &[impl AsRef<str>], available: &[&'a str]) -> Vec<&'a str> {
    let mut chosen = Vec::new();
    for wanted in languages {
        let Some(wanted) = parse(wanted.as_ref()) else {
            continue;
        };
        let parsed = available
            .iter()
            .filter_map(|tag| Some((*tag, tag.parse::<LanguageIdentifier>().ok()?)));
        let same_language: Vec<_> = parsed
            .filter(|(_, language)| language.language == wanted.language)
            .collect();
        // The very language, or else the language alone ("fr" for Belgian French), or else
        // its first variant ("pt-BR" for Portuguese), unless it's written differently.
        let best = same_language
            .iter()
            .find(|(_, language)| {
                language.region == wanted.region && language.script == wanted.script
            })
            .or_else(|| {
                same_language
                    .iter()
                    .find(|(_, language)| language.region.is_none() && language.script.is_none())
            })
            .or_else(|| {
                same_language
                    .iter()
                    .find(|(_, language)| script(language) == script(&wanted))
            });
        if let Some((tag, _)) = best
            && !chosen.contains(tag)
        {
            chosen.push(*tag);
        }
    }
    if let Some(source) = available.iter().find(|tag| **tag == SOURCE)
        && !chosen.contains(source)
    {
        chosen.push(source);
    }
    chosen
}

/// A language from a BCP 47 tag or a POSIX locale name, as `fr_CA.UTF-8@euro`: `None` for
/// the C locale, which names no language.
fn parse(name: &str) -> Option<LanguageIdentifier> {
    let name = name.split(['.', '@']).next().unwrap_or_default();
    if name.is_empty() || name == "C" || name == "POSIX" {
        return None;
    }
    name.replace('_', "-").parse().ok()
}

/// How a language is written, when it's written more than one way: Chinese is written in
/// traditional characters in Taiwan, Hong Kong and Macao.
fn script(language: &LanguageIdentifier) -> Option<&str> {
    if language.language.as_str() != "zh" {
        return None;
    }
    if let Some(script) = &language.script {
        return Some(script.as_str());
    }
    match language
        .region
        .as_ref()
        .map(unic_langid::subtags::Region::as_str)
    {
        Some("TW" | "HK" | "MO") => Some("Hant"),
        _ => Some("Hans"),
    }
}

/// The core's own translations.
const TRANSLATIONS: [(&str, &str); 1] = [(SOURCE, include_str!("../locales/en-US.ftl"))];

static CORE: RwLock<Option<Catalog>> = RwLock::new(None);

/// Have the core speak the first of `languages` it can, best first, as BCP 47 tags or POSIX
/// locale names. Until then, it speaks US English.
pub fn set_languages(languages: &[impl AsRef<str>]) {
    let catalog = Catalog::new(&TRANSLATIONS, languages);
    *CORE.write().unwrap_or_else(PoisonError::into_inner) = Some(catalog);
}

/// The core's message `id`, in the language set, with `args`.
pub(crate) fn text(id: &str, args: Option<&FluentArgs>) -> String {
    let core = CORE.read().unwrap_or_else(PoisonError::into_inner);
    if let Some(catalog) = core.as_ref() {
        return catalog.text(id, args);
    }
    drop(core);
    set_languages(&[SOURCE]);
    text(id, args)
}

/// The core's message `id`, with the arguments named: `tr!("plays", count = 3)`.
macro_rules! tr {
    ($id:expr) => {
        $crate::i18n::text($id, None)
    };
    ($id:expr, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = ::fluent_bundle::FluentArgs::new();
        $(args.set(stringify!($name), $value);)+
        $crate::i18n::text($id, Some(&args))
    }};
}
pub(crate) use tr;

#[cfg(test)]
mod tests {
    use super::*;

    const AVAILABLE: [&str; 5] = ["en-US", "fr", "fr-CA", "pt-BR", "zh-Hans"];

    #[test]
    fn picks_the_closest_languages() {
        assert_eq!(negotiate(&["fr_CA.UTF-8"], &AVAILABLE), ["fr-CA", "en-US"]);
        assert_eq!(negotiate(&["fr-BE"], &AVAILABLE), ["fr", "en-US"]);
        assert_eq!(
            negotiate(&["pt_PT", "fr"], &AVAILABLE),
            ["pt-BR", "fr", "en-US"]
        );
        assert_eq!(negotiate(&["zh_CN"], &AVAILABLE), ["zh-Hans", "en-US"]);
        assert_eq!(negotiate(&["zh_TW", "C"], &AVAILABLE), ["en-US"]);
        assert_eq!(negotiate(&["de", "en_GB"], &AVAILABLE), ["en-US"]);
    }

    #[test]
    fn falls_back_message_by_message() {
        let catalog = Catalog::new(
            &[
                ("en-US", "hello = Hello\nbye = Bye { $name }"),
                ("fr", "hello = Bonjour"),
            ],
            &["fr_FR.UTF-8"],
        );
        assert_eq!(catalog.text("hello", None), "Bonjour");
        let mut args = FluentArgs::new();
        args.set("name", "Zaz");
        assert_eq!(catalog.text("bye", Some(&args)), "Bye Zaz");
        assert_eq!(catalog.text("missing", None), "missing");
    }

    #[test]
    fn has_every_text_it_uses() {
        let catalog = Catalog::new(&TRANSLATIONS, &[SOURCE]);
        let months =
            (1..=12).flat_map(|month| [format!("month-{month}"), format!("month-short-{month}")]);
        let mut used = used_ids(include_str!("format.rs"));
        used.extend(used_ids(include_str!("library.rs")));
        assert!(used.len() > 20);
        for id in used.into_iter().chain(months) {
            assert!(catalog.has(&id), "no text {id}");
        }
    }

    /// The ids `tr!` is called with in `source`.
    fn used_ids(source: &str) -> Vec<String> {
        source
            .split("tr!(")
            .skip(1)
            .filter_map(|rest| {
                let rest = rest.trim_start().strip_prefix('"')?;
                Some(rest.split_once('"')?.0.to_owned())
            })
            .collect()
    }
}
