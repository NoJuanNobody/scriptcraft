//! Embedded UI catalogues. Add a locale file and one registry entry below to add a language.
//! English source strings are stable keys and fallback text; document text and command IDs stay unchanged.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

macro_rules! languages {
    ($($variant:ident, $code:literal, $name:literal, $file:literal);+ $(;)?) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum Language {
            #[default]
            #[serde(rename = "en")]
            English,
            $(#[serde(rename = $code)] $variant,)+
        }
        impl Language {
            pub const ALL: &'static [Self] = &[Self::English, $(Self::$variant,)+];
            pub fn code(self) -> &'static str {
                match self { Self::English => "en", $(Self::$variant => $code,)+ }
            }
            pub fn name(self) -> &'static str {
                match self { Self::English => "English", $(Self::$variant => $name,)+ }
            }
            fn source(self) -> &'static str {
                match self { Self::English => include_str!("../locales/en.json"), $(Self::$variant => include_str!($file),)+ }
            }
        }
    };
}
languages! {
    Russian, "ru", "Русский", "../locales/ru.json";
}

impl Language {
    fn catalog(self) -> &'static BTreeMap<String, String> {
        static CATALOGS: OnceLock<BTreeMap<Language, BTreeMap<String, String>>> = OnceLock::new();
        static EMPTY: OnceLock<BTreeMap<String, String>> = OnceLock::new();
        CATALOGS
            .get_or_init(|| Self::ALL.iter().map(|language| (*language, serde_json::from_str(language.source()).unwrap_or_default())).collect())
            .get(&self)
            .unwrap_or_else(|| EMPTY.get_or_init(BTreeMap::new))
    }

    pub fn text(self, english: &str) -> &str {
        self.catalog().get(english).map_or(english, String::as_str)
    }

    pub fn format(self, key: &str, values: &[(&str, String)]) -> String {
        render(self.catalog(), key, values)
    }

    pub fn status(self, message: &str) -> String {
        for (caption, key, name) in [
            ("Opened ", "Opened {name}", "name"),
            ("Saved ", "Saved {name}", "name"),
            ("Exported the breakdown to ", "Exported the breakdown to {path}", "path"),
        ] {
            if let Some(value) = message.strip_prefix(caption) {
                return self.format(key, &[(name, value.into())]);
            }
        }
        self.text(message).to_string()
    }

    pub fn category(self, name: &str) -> &str {
        if scriptcraft_doc::TAG_CATEGORIES.iter().any(|(standard, _)| *standard == name) { self.text(name) } else { name }
    }
}

fn render(catalog: &BTreeMap<String, String>, key: &str, values: &[(&str, String)]) -> String {
    let mut rest = catalog.get(key).map_or(key, String::as_str);
    let mut out = String::new();
    while let Some((before, tail)) = rest.split_once('{') {
        out.push_str(before);
        let Some((name, after)) = tail.split_once('}') else {
            out.push('{');
            out.push_str(tail);
            return out;
        };
        if let Some((_, value)) = values.iter().find(|(key, _)| *key == name) {
            out.push_str(value);
        } else {
            out.push('{');
            out.push_str(name);
            out.push('}');
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn catalogs_have_matching_keys_and_placeholders() {
        let english: BTreeMap<String, String> = serde_json::from_str(Language::English.source()).unwrap();
        for language in Language::ALL {
            let catalog: BTreeMap<String, String> = serde_json::from_str(language.source()).unwrap();
            assert_eq!(catalog.keys().collect::<Vec<_>>(), english.keys().collect::<Vec<_>>());
            for (key, value) in &catalog {
                assert_eq!(placeholders(value), placeholders(&english[key]), "{language:?}: {key}");
            }
        }
    }

    fn placeholders(template: &str) -> BTreeSet<&str> {
        template.split('{').skip(1).filter_map(|s| s.split_once('}').map(|(name, _)| name)).collect()
    }

    #[test]
    fn formatting_is_named_and_values_are_never_reinterpreted() {
        let catalog = BTreeMap::from([("test".into(), "{second} then {first}: {second}".into())]);
        assert_eq!(render(&catalog, "test", &[("first", "{second}".into()), ("second", "Ёжик".into())]), "Ёжик then {second}: Ёжик");
        assert_eq!(render(&BTreeMap::new(), "Missing {name}", &[("name", "Алёна".into())]), "Missing Алёна");
        assert_eq!(Language::Russian.format("Page {page} of {total}", &[("page", "2".into()), ("total", "5".into())]), "Страница 2 из 5");
    }

    #[test]
    fn english_default_unknown_labels_and_file_names_are_preserved() {
        assert_eq!(Language::default().text("Save"), "Save");
        assert_eq!(Language::Russian.text("Save"), "Сохранить");
        assert_eq!(Language::Russian.text("User script"), "User script");
        assert_eq!(Language::Russian.status("Opened Save Ёжик.fountain"), "Открыт Save Ёжик.fountain");
        assert_eq!(Language::English.status("Saved сценарий.fdx"), "Saved сценарий.fdx");
        assert_eq!(Language::Russian.status("bad parameters: path"), "bad parameters: path");
        assert_eq!(serde_json::to_value(Language::Russian).unwrap(), "ru");
        assert!(serde_json::from_str::<Language>("\"unknown\"").is_err());
    }
}
