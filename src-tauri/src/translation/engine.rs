//! NllbEngine: loads NLLB-200 int8 (CTranslate2) and translates text on CPU.
//! CPU int8 only — NEVER enable Metal here.
use super::languages::split_sentences;
use crate::error::{AppError, StorageErrorCode};
use ct2rs::tokenizers::hf::Tokenizer;
use ct2rs::{Config, Translator};
use std::path::Path;

pub struct NllbEngine {
    translator: Translator<Tokenizer>,
}

impl NllbEngine {
    pub fn load(model_path: &Path) -> Result<Self, AppError> {
        // The converted NLLB tokenizer's static post-processor ends every
        // source with `<unk>`. That happens because a tokenizer.json cannot
        // hold a runtime source-language choice. Disable that processor and
        // append `</s><source-language>` ourselves in translate().
        let mut tokenizer = Tokenizer::new(model_path).map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("load NLLB tokenizer: {e}"),
        })?;
        tokenizer.disable_spacial_token();
        let translator = Translator::with_tokenizer(model_path, tokenizer, &Config::default())
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("load NLLB model: {e}"),
            })?;
        Ok(Self { translator })
    }

    /// Translate each input text as a whole (sentence-split internally, re-joined).
    /// Output vec aligns 1:1 with `texts`.
    ///
    /// ct2rs accepts source strings and target language prefixes. NLLB does not
    /// auto-detect its source language: its encoder input must end with
    /// `</s><source FLORES code>`. The tokenizer is configured without its
    /// static special-token processor in load(), so the runtime source code in
    /// this method is authoritative.
    /// `translate_batch_with_target_prefix` returns Vec<(String, Option<f32>)>.
    pub fn translate(
        &self,
        texts: &[String],
        src_lang: &str,
        tgt_lang: &str,
    ) -> Result<Vec<String>, AppError> {
        let mut out = Vec::with_capacity(texts.len());
        for text in texts {
            let sentences = split_sentences(text);
            if sentences.is_empty() {
                out.push(String::new());
                continue;
            }
            let sources: Vec<String> = sentences
                .iter()
                .map(|sentence| format!("{sentence}</s>{src_lang}"))
                .collect();
            // Target language as a target prefix, one per source sentence.
            let target_prefixes: Vec<Vec<String>> = sentences
                .iter()
                .map(|_| vec![tgt_lang.to_string()])
                .collect();
            let results = self
                .translator
                .translate_batch_with_target_prefix(
                    &sources,
                    &target_prefixes,
                    &Default::default(),
                    None,
                )
                .map_err(|e| AppError::StorageError {
                    code: StorageErrorCode::DatabaseError,
                    message: format!("nllb translate: {e}"),
                })?;
            // results: Vec<(String, Option<f32>)> — (translated_text, score).
            let joined = results
                .into_iter()
                .map(|(text, _score)| text)
                .collect::<Vec<_>>()
                .join(" ");
            out.push(joined);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    #[ignore] // requires model at /tmp/nllb-smoke/model
    fn translates_english_to_bangla() {
        let engine = NllbEngine::load(&PathBuf::from("/tmp/nllb-smoke/model")).unwrap();
        let out = engine
            .translate(&["Hello, how are you?".to_string()], "eng_Latn", "ben_Beng")
            .unwrap();
        assert_eq!(out.len(), 1);
        assert!(!out[0].is_empty());
        // Assert real Bengali script (U+0980–U+09FF), not English pass-through.
        assert!(
            out[0]
                .chars()
                .any(|c| ('\u{0980}'..='\u{09FF}').contains(&c)),
            "translation contains no Bengali characters: {:?}",
            out[0]
        );
        println!("ben: {}", out[0]);

        let from_arabic = engine
            .translate(
                &["مرحبا، كيف حالك اليوم؟".to_string()],
                "arb_Arab",
                "ben_Beng",
            )
            .unwrap();
        assert!(
            from_arabic[0]
                .chars()
                .any(|c| ('\u{0980}'..='\u{09FF}').contains(&c)),
            "Arabic translation contains no Bengali characters: {:?}",
            from_arabic[0]
        );
        println!("arb->ben: {}", from_arabic[0]);
    }
}
