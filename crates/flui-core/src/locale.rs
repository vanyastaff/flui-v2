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
