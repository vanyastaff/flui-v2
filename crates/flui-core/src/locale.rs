use crate::Global;

/// System locale — language and optional country code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locale {
    /// ISO 639-1 language code: "en", "ru", "ar", "he"
    pub language: String,
    /// ISO 3166-1 country code: "US", "RU", "SA"
    pub country: Option<String>,
}

impl Default for Locale {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            country: None,
        }
    }
}

impl Locale {
    /// Create a new locale with the given language and optional country.
    pub fn new(language: impl Into<String>, country: Option<impl Into<String>>) -> Self {
        Self {
            language: language.into(),
            country: country.map(|c| c.into()),
        }
    }

    /// Parse POSIX locale: "en_US.UTF-8" or "ru_RU" or "en"
    pub fn from_posix(s: &str) -> Self {
        let without_encoding = s.split('.').next().unwrap_or("en");
        let without_modifier = without_encoding.split('@').next().unwrap_or("en");
        if let Some((lang, country)) = without_modifier.split_once('_') {
            Self::new(lang.to_lowercase(), Some(country.to_uppercase()))
        } else {
            Self::new(without_modifier.to_lowercase(), None::<String>)
        }
    }

    /// Parse BCP 47 tag: "en-US" or "zh-Hans-CN"
    pub fn from_bcp47(s: &str) -> Self {
        let parts: Vec<&str> = s.split('-').collect();
        let language = parts.first().unwrap_or(&"en").to_lowercase();
        let country = parts
            .iter()
            .skip(1)
            .find(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_uppercase()));
        Self::new(language, country.map(|c| c.to_string()))
    }
}

/// Text direction — left-to-right or right-to-left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextDirection {
    /// Left-to-right text direction.
    #[default]
    Ltr,
    /// Right-to-left text direction.
    Rtl,
}

impl TextDirection {
    /// Determine text direction from a language code.
    pub fn from_language(language: &str) -> Self {
        match language {
            "ar" | "he" | "fa" | "ur" | "ps" | "sd" | "yi" | "ckb" | "ug" => Self::Rtl,
            _ => Self::Ltr,
        }
    }
}

/// App-level global storing the system locale.
pub(crate) struct SystemLocale(pub Locale);
impl Global for SystemLocale {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_from_posix_full() {
        let locale = Locale::from_posix("en_US.UTF-8");
        assert_eq!(locale.language, "en");
        assert_eq!(locale.country, Some("US".to_string()));
    }

    #[test]
    fn test_locale_from_posix_no_encoding() {
        let locale = Locale::from_posix("ru_RU");
        assert_eq!(locale.language, "ru");
        assert_eq!(locale.country, Some("RU".to_string()));
    }

    #[test]
    fn test_locale_from_posix_language_only() {
        let locale = Locale::from_posix("en");
        assert_eq!(locale.language, "en");
        assert_eq!(locale.country, None);
    }

    #[test]
    fn test_locale_from_posix_with_modifier() {
        let locale = Locale::from_posix("sr_RS@latin");
        assert_eq!(locale.language, "sr");
        assert_eq!(locale.country, Some("RS".to_string()));
    }

    #[test]
    fn test_locale_from_bcp47_simple() {
        let locale = Locale::from_bcp47("en-US");
        assert_eq!(locale.language, "en");
        assert_eq!(locale.country, Some("US".to_string()));
    }

    #[test]
    fn test_locale_from_bcp47_with_script() {
        let locale = Locale::from_bcp47("zh-Hans-CN");
        assert_eq!(locale.language, "zh");
        assert_eq!(locale.country, Some("CN".to_string()));
    }

    #[test]
    fn test_text_direction_ltr() {
        assert_eq!(TextDirection::from_language("en"), TextDirection::Ltr);
        assert_eq!(TextDirection::from_language("ru"), TextDirection::Ltr);
        assert_eq!(TextDirection::from_language("zh"), TextDirection::Ltr);
    }

    #[test]
    fn test_text_direction_rtl() {
        assert_eq!(TextDirection::from_language("ar"), TextDirection::Rtl);
        assert_eq!(TextDirection::from_language("he"), TextDirection::Rtl);
        assert_eq!(TextDirection::from_language("fa"), TextDirection::Rtl);
        assert_eq!(TextDirection::from_language("ur"), TextDirection::Rtl);
    }
}
