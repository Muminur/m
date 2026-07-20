//! Offline neural machine translation via NLLB-200 (CTranslate2 / ct2rs).
//! CPU int8 only — NEVER enable Metal/GPU here (Intel x86_64 aborts on Metal).
//!
//! CONFIRMED ct2rs API (ct2rs 0.9.22, default-features off, features
//! `dnnl` + `ruy` + `tokenizers` — verified by the passing smoke test below
//! against facebook/nllb-200-distilled-600M int8).
//! NOTE: do NOT use the `ct2rs-platform` wrapper — its `intel-onemkl-prebuild`
//! dependency panics "MacOS is not supported" on x86_64. Depend on `ct2rs`
//! directly and select the macOS-safe CPU backends `dnnl` + `ruy`.
//!
//!   use ct2rs::{Config, Translator};
//!
//!   // Constructor: model dir must contain model.bin, config.json, and a
//!   // tokenizer.json (the `tokenizers` feature reads tokenizer.json).
//!   let translator = Translator::new(model_dir, &Config::default())?;  // -> anyhow::Result<Translator<Tokenizer>>
//!
//!   // Translate: sources are PLAIN strings (the built-in tokenizer handles
//!   // encoding — do NOT pre-tokenize). The NLLB target language is passed as a
//!   // target prefix: one Vec<String> of language tokens per source sentence.
//!   let sources: Vec<String> = vec!["Hello, how are you?".to_string()];
//!   let target_prefixes: Vec<Vec<String>> = vec![vec!["ben_Beng".to_string()]];
//!   let results = translator.translate_batch_with_target_prefix(
//!       &sources,
//!       &target_prefixes,
//!       &Default::default(),   // ct2rs::TranslationOptions
//!       None,                  // Option<&mut dyn FnMut(GenerationStepResult) -> Result<()>> step callback
//!   )?;
//!   // results: Vec<(String, Option<f32>)> — (translated_text, score) per source.
//!
//! NLLB language codes are FLORES-200 style: eng_Latn (English), ben_Beng
//! (Bengali), arb_Arab (Modern Standard Arabic). Source language is auto-detected
//! by NLLB; only the target prefix is required.

#[cfg(test)]
mod smoke {
    use ct2rs::{Config, Translator};

    /// Smoke test: prove ct2rs builds and translates English -> Bengali on CPU int8.
    /// Requires the pre-converted NLLB-200-distilled-600M int8 model at
    /// /tmp/nllb-smoke/model (see Task 1 brief, Step 5). Run explicitly:
    ///   cargo test -p whisper-desk-app translation::smoke::eng_to_ben_smoke \
    ///       -- --ignored --nocapture
    #[test]
    #[ignore] // requires the model at /tmp/nllb-smoke/model; run explicitly
    fn eng_to_ben_smoke() {
        let path = "/tmp/nllb-smoke/model";
        let translator =
            Translator::new(path, &Config::default()).expect("load NLLB model");

        // Sources are plain strings; the built-in tokenizer encodes them.
        let sources = vec!["Hello, how are you?".to_string()];
        // NLLB target language passed as a target prefix (one per source).
        let target_prefixes = vec![vec!["ben_Beng".to_string()]];

        let results = translator
            .translate_batch_with_target_prefix(
                &sources,
                &target_prefixes,
                &Default::default(),
                None,
            )
            .expect("translate");

        println!("NLLB eng->ben: {results:?}");

        assert!(!results.is_empty(), "no translation returned");
        let (text, _score) = &results[0];
        assert!(!text.trim().is_empty(), "empty translation text");
        // Assert the output is actually Bengali script (Unicode block U+0980–U+09FF),
        // not English pass-through or a mock.
        assert!(
            text.chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c)),
            "translation contains no Bengali characters: {text:?}"
        );
    }
}
