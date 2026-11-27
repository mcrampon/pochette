//! The language Pochette speaks: the one picked in Settings, or else the system's. Picked
//! there, it's taken up the next time Pochette opens.
//!
//! The texts are in `locales/<language>.ftl`, as [`pochette_core::i18n`] has them.

use std::sync::OnceLock;

use pochette_core::i18n::{self, Catalog, SOURCE};
use relm4::gtk::glib;

/// The app's own translations, by language tag; the core has its own.
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

static CATALOG: OnceLock<Catalog> = OnceLock::new();

/// Speak `language`, a BCP 47 tag, or the system's languages when `None`. Once, before any
/// text is shown.
pub fn init(language: Option<&str>) {
    let languages: Vec<String> = match language {
        Some(language) => vec![language.to_owned()],
        None => glib::language_names()
            .into_iter()
            .map(String::from)
            .collect(),
    };
    i18n::set_languages(&languages);
    if CATALOG
        .set(Catalog::new(&TRANSLATIONS, &languages))
        .is_err()
    {
        tracing::warn!("the language was already set");
    }
}

/// The message `id`, with `args` in its placeholders. Use [`tr!`].
pub fn text(id: &str, args: Option<&i18n::FluentArgs>) -> String {
    CATALOG
        .get_or_init(|| Catalog::new(&TRANSLATIONS, &[SOURCE]))
        .text(id, args)
}

/// The message `id` in `locales/`, with the arguments named: `tr!("album-count", count = 3)`.
macro_rules! tr {
    ($id:expr) => {
        $crate::i18n::text($id, None)
    };
    ($id:expr, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = ::pochette_core::i18n::FluentArgs::new();
        $(args.set(stringify!($name), $value);)+
        $crate::i18n::text($id, Some(&args))
    }};
}
pub(crate) use tr;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// Every `tr!` in the sources has its text in US English.
    #[test]
    fn has_every_text_it_uses() {
        let catalog = Catalog::new(&TRANSLATIONS, &[SOURCE]);
        let mut used = Vec::new();
        collect_ids(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path(),
            &mut used,
        );
        assert!(used.len() > 100, "found only {} texts", used.len());
        for (file, id) in used {
            assert!(catalog.has(&id), "no text {id}, used in {file}");
        }
    }

    /// Every translation in `locales` is whole and sound.
    #[test]
    fn translations_are_sound() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
        let source = std::fs::read_to_string(dir.join("en-US.ftl")).unwrap();
        let mut found = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let translation = std::fs::read_to_string(&path).unwrap();
            found.extend(
                i18n::problems(&source, &translation)
                    .into_iter()
                    .map(|problem| format!("{}: {problem}", path.display())),
            );
        }
        assert!(found.is_empty(), "{}", found.join("\n"));
    }

    /// Every message of every translation formats.
    #[test]
    fn translations_format() {
        let mut found = Vec::new();
        for (tag, translation) in TRANSLATIONS {
            found.extend(i18n::formatting_problems(
                TRANSLATIONS[0].1,
                tag,
                translation,
            ));
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

    fn collect_ids(dir: &Path, ids: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect_ids(&path, ids);
            } else if path.extension().is_some_and(|extension| extension == "rs")
                && !path.ends_with("i18n.rs")
            {
                let source = std::fs::read_to_string(&path).unwrap();
                let file = path.display().to_string();
                ids.extend(source.match_indices("tr!(").filter_map(|(at, call)| {
                    // Not include_str!(…).
                    let before = source[..at].chars().next_back();
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        return None;
                    }
                    let rest = source[at + call.len()..].trim_start().strip_prefix('"')?;
                    Some((file.clone(), rest.split_once('"')?.0.to_owned()))
                }));
            }
        }
    }
}
