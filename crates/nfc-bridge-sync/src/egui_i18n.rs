//! Sistema i18n nativo respaldado por los diccionarios externos `ui/i18n/en.json`
//! (idioma principal de desarrollo) y `ui/i18n/es.json`.

use crate::config::EmbeddedUi;
use std::collections::HashMap;

pub struct I18nCatalog {
    en: HashMap<String, String>,
    es: HashMap<String, String>,
    system_is_spanish: bool,
}

impl I18nCatalog {
    pub fn load() -> Self {
        let en = serde_json::from_str::<HashMap<String, String>>(EmbeddedUi::EN_I18N_JSON)
            .unwrap_or_default();
        let es = serde_json::from_str::<HashMap<String, String>>(EmbeddedUi::ES_I18N_JSON)
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

fn detect_windows_spanish_locale() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
        let mut buf = [0u16; 85];
        let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
        if len > 1 {
            let s = String::from_utf16_lossy(&buf[..(len as usize - 1)]).to_ascii_lowercase();
            return s.starts_with("es");
        }
    }
    false
}
