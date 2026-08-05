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
//! NLLB language codes are FLORES-200 style: eng_Latn (English), ben_Beng
//! (Bengali), arb_Arab (Modern Standard Arabic). Callers provide both source and
//! target codes; NLLB does not auto-detect its encoder language. [`NllbEngine`]
//! configures the HuggingFace tokenizer with the runtime source suffix and the
//! CTranslate2 decoder with the target-language prefix.
//!
//! [`NllbEngine`]: engine::NllbEngine

pub mod engine;
pub mod languages;
pub mod manager;
pub mod model;

#[cfg(test)]
mod smoke {
    use super::engine::NllbEngine;
    use std::path::Path;

    /// Smoke test: prove ct2rs builds and translates English -> Bengali on CPU int8.
    /// Requires the pre-converted NLLB-200-distilled-600M int8 model at
    /// /tmp/nllb-smoke/model (see Task 1 brief, Step 5). Run explicitly:
    ///   cargo test -p whisper-desk-app translation::smoke::eng_to_ben_smoke \
    ///       -- --ignored --nocapture
    #[test]
    #[ignore] // requires the model at /tmp/nllb-smoke/model; run explicitly
    fn eng_to_ben_smoke() {
        let path = "/tmp/nllb-smoke/model";
        let engine = NllbEngine::load(Path::new(path)).expect("load NLLB model");
        let results = engine
            .translate(&["Hello, how are you?".into()], "eng_Latn", "ben_Beng")
            .expect("translate");

        println!("NLLB eng->ben: {results:?}");

        assert!(!results.is_empty(), "no translation returned");
        let text = &results[0];
        assert!(!text.trim().is_empty(), "empty translation text");
        // Assert the output is actually Bengali script (Unicode block U+0980–U+09FF),
        // not English pass-through or a mock.
        assert!(
            text.chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c)),
            "translation contains no Bengali characters: {text:?}"
        );
    }
}
