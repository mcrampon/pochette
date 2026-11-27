//! The language Pochette speaks: the one picked in Settings, or else the system's. Picked
//! there, it's taken up the next time Pochette opens.
//!
//! The texts are in `locales/<language>.ftl`, as [`pochette_core::i18n`] has them.

use std::sync::OnceLock;

use pochette_core::i18n::{self, Catalog, SOURCE};
use relm4::gtk::glib;

/// The app's own translations, by language tag; the core has its own.
const TRANSLATIONS: [(&str, &str); 1] = [(SOURCE, include_str!("../locales/en-US.ftl"))];

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
