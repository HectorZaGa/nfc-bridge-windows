//! Diccionarios i18n incrustados desde `ui/i18n/en.json` (principal) y `ui/i18n/es.json`.

use crate::config::EmbeddedI18n;
use crate::platform::detect_windows_spanish_locale;
use std::collections::HashMap;

pub struct I18nCatalog {
    en: HashMap<String, String>,
    es: HashMap<String, String>,
    system_is_spanish: bool,
}

impl I18nCatalog {
    pub fn load() -> Self {
        let en = serde_json::from_str::<HashMap<String, String>>(EmbeddedI18n::EN_JSON)
            .unwrap_or_default();
        let es = serde_json::from_str::<HashMap<String, String>>(EmbeddedI18n::ES_JSON)
            .unwrap_or_default();
        Self {
            en,
            es,
            system_is_spanish: detect_windows_spanish_locale(),
        }
    }

    pub fn resolve_lang_code(&self, pref: &str) -> &'static str {
        match pref.trim().to_ascii_lowercase().as_str() {
            "en" => "en",
            "es" => "es",
            _ => {
                if self.system_is_spanish {
                    "es"
                } else {
                    "en"
                }
            }
        }
    }

    pub fn t<'a>(&'a self, pref: &str, key: &'a str) -> &'a str {
        let dict = if self.resolve_lang_code(pref) == "es" {
            &self.es
        } else {
            &self.en
        };
        dict.get(key)
            .map(String::as_str)
            .or_else(|| self.en.get(key).map(String::as_str))
            .unwrap_or(key)
    }

    pub fn t_vars(&self, pref: &str, key: &str, vars: &[(&str, &str)]) -> String {
        let mut s = self.t(pref, key).to_string();
        for (k, v) in vars {
            let needle = format!("{{{}}}", k);
            s = s.replace(&needle, v);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_i18n_catalogs_are_valid_and_in_sync() {
        let en: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(EmbeddedI18n::EN_JSON).expect("en.json should parse");
        let es: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(EmbeddedI18n::ES_JSON).expect("es.json should parse");

        for required in [
            "title_options_sync",
            "title_options_tools",
            "settings_appearance_header",
            "settings_language_system",
            "settings_language_en",
            "settings_language_es",
            "settings_mode_portable",
            "nav_appearance",
        ] {
            assert!(en.contains_key(required), "en.json missing {required}");
            assert!(es.contains_key(required), "es.json missing {required}");
        }

        let en_keys: std::collections::BTreeSet<_> = en.keys().cloned().collect();
        let es_keys: std::collections::BTreeSet<_> = es.keys().cloned().collect();
        assert_eq!(en_keys, es_keys, "en.json and es.json key sets must match");
    }
}
