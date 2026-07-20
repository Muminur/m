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
        ("arb_Arab", "Arabic"),
        ("hin_Deva", "Hindi"),
        ("urd_Arab", "Urdu"),
        ("spa_Latn", "Spanish"),
        ("fra_Latn", "French"),
        ("deu_Latn", "German"),
    ]
    .iter()
    .map(|(c, l)| TranslationLanguage { code: c.to_string(), label: l.to_string() })
    .collect()
}

pub fn is_supported(flores_code: &str) -> bool {
    supported_languages().iter().any(|l| l.code == flores_code)
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
}
