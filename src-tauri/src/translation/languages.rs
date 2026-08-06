//! FLORES-200 language codes for NLLB + sentence splitting helpers.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationLanguage {
    pub code: String,  // FLORES-200 code, e.g. "ben_Beng"
    pub label: String, // human-readable, e.g. "Bangla"
}

pub fn supported_languages() -> Vec<TranslationLanguage> {
    [
        ("eng_Latn", "English"),
        ("ben_Beng", "Bangla"),
        ("zho_Hans", "Chinese (Simplified)"),
        ("hin_Deva", "Hindi"),
        ("spa_Latn", "Spanish"),
        ("arb_Arab", "Arabic"),
        ("por_Latn", "Portuguese"),
        ("rus_Cyrl", "Russian"),
        ("fra_Latn", "French"),
        ("urd_Arab", "Urdu"),
        ("jpn_Jpan", "Japanese"),
        ("deu_Latn", "German"),
        ("nld_Latn", "Dutch"),
        ("ita_Latn", "Italian"),
        ("kor_Hang", "Korean"),
    ]
    .iter()
    .map(|(c, l)| TranslationLanguage {
        code: c.to_string(),
        label: l.to_string(),
    })
    .collect()
}

pub fn is_supported(flores_code: &str) -> bool {
    supported_languages().iter().any(|l| l.code == flores_code)
}

/// Map a Whisper ISO 639-1 language code (e.g. "en", "bn", "ar") to a
/// FLORES-200 code understood by NLLB. If the input is already a FLORES-200
/// code (contains an underscore, e.g. "ben_Beng"), it is returned as-is when
/// supported. Returns `None` for unknown codes.
pub fn to_flores(code: &str) -> Option<&'static str> {
    match code {
        "en" | "eng_Latn" => Some("eng_Latn"),
        "bn" | "ben_Beng" => Some("ben_Beng"),
        "ar" | "arb_Arab" => Some("arb_Arab"),
        "hi" | "hin_Deva" => Some("hin_Deva"),
        "ur" | "urd_Arab" => Some("urd_Arab"),
        "es" | "spa_Latn" => Some("spa_Latn"),
        "fr" | "fra_Latn" => Some("fra_Latn"),
        "de" | "deu_Latn" => Some("deu_Latn"),
        "nl" | "nld_Latn" => Some("nld_Latn"),
        "pt" | "por_Latn" => Some("por_Latn"),
        "it" | "ita_Latn" => Some("ita_Latn"),
        "ru" | "rus_Cyrl" => Some("rus_Cyrl"),
        "ja" | "jpn_Jpan" => Some("jpn_Jpan"),
        "zh" | "zho_Hans" => Some("zho_Hans"),
        "ko" | "kor_Hang" => Some("kor_Hang"),
        _ => None,
    }
}

pub fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if matches!(ch, '.' | '?' | '!' | '।') {
            let t = cur.trim();
            if !t.is_empty() {
                out.push(t.to_string());
            }
            cur.clear();
        }
    }
    let tail = cur.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_bangla_arabic_english() {
        assert!(is_supported("ben_Beng"));
        assert!(is_supported("arb_Arab"));
        assert!(is_supported("eng_Latn"));
        assert!(is_supported("rus_Cyrl"));
        assert!(!is_supported("xx_Zzzz"));
    }

    #[test]
    fn splits_english_sentences() {
        let s = split_sentences("Hello there. How are you? I am fine!");
        assert_eq!(s.len(), 3);
        assert_eq!(s[0], "Hello there.");
    }

    #[test]
    fn splits_bangla_danda() {
        // Two Bangla sentences separated by danda (।)
        let s = split_sentences("আমি ভালো আছি। তুমি কেমন আছো।");
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn single_sentence_no_terminator() {
        let s = split_sentences("just one line");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0], "just one line");
    }

    #[test]
    fn to_flores_maps_iso_and_passthrough() {
        assert_eq!(to_flores("en"), Some("eng_Latn"));
        assert_eq!(to_flores("bn"), Some("ben_Beng"));
        assert_eq!(to_flores("ar"), Some("arb_Arab"));
        // Already-FLORES passthrough.
        assert_eq!(to_flores("ben_Beng"), Some("ben_Beng"));
        assert_eq!(to_flores("nl"), Some("nld_Latn"));
        assert_eq!(to_flores("pt"), Some("por_Latn"));
        assert_eq!(to_flores("it"), Some("ita_Latn"));
        assert_eq!(to_flores("ja"), Some("jpn_Jpan"));
        assert_eq!(to_flores("ru"), Some("rus_Cyrl"));
        assert_eq!(to_flores("zh"), Some("zho_Hans"));
        assert_eq!(to_flores("ko"), Some("kor_Hang"));
        // Unknown.
        assert_eq!(to_flores("zz"), None);
    }
}
