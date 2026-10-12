//! The language of messages shown to the user. Only messages: data written into a
//! project (default names and the like) stays the same in every language, so replaying
//! a file gives the same project wherever it is opened.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    /// Vietnamese, the app's original language.
    #[default]
    Vi,
    En,
}

impl Lang {
    /// The language for a code such as `"vi"` or `"en"`.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "vi" => Some(Self::Vi),
            "en" => Some(Self::En),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Vi => "vi",
            Self::En => "en",
        }
    }

    /// Pick the text for this language.
    pub fn pick<T>(self, vi: T, en: T) -> T {
        match self {
            Self::Vi => vi,
            Self::En => en,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_match_serde() {
        for lang in [Lang::Vi, Lang::En] {
            assert_eq!(Lang::from_code(lang.code()), Some(lang));
            assert_eq!(
                serde_json::to_string(&lang).unwrap(),
                format!("\"{}\"", lang.code())
            );
        }
        assert_eq!(Lang::from_code("fr"), None);
        assert_eq!(Lang::default(), Lang::Vi);
    }
}
