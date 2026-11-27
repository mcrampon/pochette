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

/// What's wrong with `translation`, a language's `.ftl` file, next to `source`, the US
/// English one: errors, and messages it lacks, or that US English doesn't have, or that use
/// values US English doesn't give them: those it uses, or its comment names. For the tests
/// of each app's texts.
#[doc(hidden)]
#[must_use]
pub fn problems(source: &str, translation: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let (source, translation) = (
        parse_ftl(source, &mut problems),
        parse_ftl(translation, &mut problems),
    );
    for (id, variables) in &translation {
        match source.iter().find(|(source_id, _)| source_id == id) {
            None => problems.push(format!("{id}: not in US English")),
            Some((_, source_variables)) => {
                for variable in variables {
                    if !source_variables.contains(variable) {
                        problems.push(format!("{id}: no ${variable} in US English"));
                    }
                }
            }
        }
    }
    for (id, _) in &source {
        if !translation.iter().any(|(translated, _)| translated == id) {
            problems.push(format!("{id}: missing"));
        }
    }
    problems
}

/// The errors in formatting each message of `translation`, the `.ftl` file of the language
/// `tag`, with every value US English `source` gives it, numbers as 1, 2, 5 and 22 in turn.
/// For the tests of each app's texts.
#[doc(hidden)]
#[must_use]
pub fn formatting_problems(source: &str, tag: &str, translation: &str) -> Vec<String> {
    /// The values that are numbers, which plurals pick their form from.
    const NUMBERS: [&str; 10] = [
        "count", "days", "weeks", "minutes", "hours", "bits", "day", "number", "port", "decade",
    ];
    let mut problems = Vec::new();
    let messages = parse_ftl(source, &mut problems);
    let Some(bundle) = bundle(tag, translation) else {
        return vec![format!("{tag}: not a language tag")];
    };
    for (id, variables) in messages {
        let Some(pattern) = bundle.get_message(&id).and_then(|message| message.value()) else {
            continue;
        };
        for number in [1, 2, 5, 22] {
            let mut args = FluentArgs::new();
            for variable in &variables {
                if NUMBERS.contains(&variable.as_str()) {
                    args.set(variable.clone(), number);
                } else {
                    args.set(variable.clone(), "x");
                }
            }
            let mut errors = Vec::new();
            let text = bundle.format_pattern(pattern, Some(&args), &mut errors);
            if !errors.is_empty() || text.is_empty() {
                problems.push(format!("{id} with {number}: {errors:?} gives {text:?}"));
            }
        }
    }
    problems
}

/// The messages of an `.ftl` file, with the variables each uses.
fn parse_ftl(text: &str, problems: &mut Vec<String>) -> Vec<(String, Vec<String>)> {
    use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};

    fn pattern_variables(pattern: &Pattern<&str>, variables: &mut Vec<String>) {
        for element in &pattern.elements {
            if let PatternElement::Placeable { expression } = element {
                expression_variables(expression, variables);
            }
        }
    }
    fn expression_variables(expression: &Expression<&str>, variables: &mut Vec<String>) {
        match expression {
            Expression::Select { selector, variants } => {
                inline_variables(selector, variables);
                for variant in variants {
                    pattern_variables(&variant.value, variables);
                }
            }
            Expression::Inline(inline) => inline_variables(inline, variables),
        }
    }
    fn inline_variables(inline: &InlineExpression<&str>, variables: &mut Vec<String>) {
        match inline {
            InlineExpression::VariableReference { id } => variables.push(id.name.to_owned()),
            InlineExpression::Placeable { expression } => {
                expression_variables(expression, variables);
            }
            _ => {}
        }
    }

    let resource = match fluent_syntax::parser::parse(text) {
        Ok(resource) => resource,
        Err((resource, errors)) => {
            problems.extend(errors.iter().map(|error| format!("syntax: {error:?}")));
            resource
        }
    };
    let mut messages = Vec::new();
    for entry in &resource.body {
        match entry {
            Entry::Message(message) => {
                let mut variables = Vec::new();
                if let Some(pattern) = &message.value {
                    pattern_variables(pattern, &mut variables);
                }
                // Those the comment names are given too, for the translations that need them.
                for line in message.comment.iter().flat_map(|comment| &comment.content) {
                    variables.extend(line.split('$').skip(1).map(|rest| {
                        rest.chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                            .collect::<String>()
                    }));
                }
                messages.push((message.id.name.to_owned(), variables));
            }
            Entry::Junk { content } => problems.push(format!("unreadable: {content}")),
            _ => {}
        }
    }
    messages
}

/// The core's own translations.
const TRANSLATIONS: [(&str, &str); 10] = [
    (SOURCE, include_str!("../locales/en-US.ftl")),
    ("de", include_str!("../locales/de.ftl")),
    ("en-GB", include_str!("../locales/en-GB.ftl")),
    ("es", include_str!("../locales/es.ftl")),
    ("fr", include_str!("../locales/fr.ftl")),
    ("fr-CA", include_str!("../locales/fr-CA.ftl")),
    ("it", include_str!("../locales/it.ftl")),
    ("pl", include_str!("../locales/pl.ftl")),
    ("pt-BR", include_str!("../locales/pt-BR.ftl")),
    ("zh-Hans", include_str!("../locales/zh-Hans.ftl")),
];

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
    use std::path::Path;

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

    /// Every translation in `locales` is whole and sound.
    #[test]
    fn translations_are_sound() {
        check_translations(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("locales")
                .as_path(),
        );
    }

    /// Every message of every translation formats.
    #[test]
    fn translations_format() {
        let mut found = Vec::new();
        for (tag, translation) in TRANSLATIONS {
            found.extend(formatting_problems(TRANSLATIONS[0].1, tag, translation));
        }
        assert!(found.is_empty(), "{}", found.join("\n"));
    }

    /// Every translation in `locales` is built in.
    #[test]
    fn has_every_translation() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
        for entry in std::fs::read_dir(dir).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            let tag = name.trim_end_matches(".ftl");
            assert!(
                TRANSLATIONS.iter().any(|(built_in, _)| *built_in == tag),
                "{name} isn't built in"
            );
        }
    }

    /// Check each `.ftl` file in `dir` against its US English one.
    fn check_translations(dir: &Path) {
        let source = std::fs::read_to_string(dir.join("en-US.ftl")).unwrap();
        let mut found = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let translation = std::fs::read_to_string(&path).unwrap();
            found.extend(
                problems(&source, &translation)
                    .into_iter()
                    .map(|problem| format!("{}: {problem}", path.display())),
            );
        }
        assert!(found.is_empty(), "{}", found.join("\n"));
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
