//! NllbEngine: loads NLLB-200 int8 (CTranslate2) and translates text on CPU.
//! CPU int8 only — NEVER enable Metal here.
use std::path::Path;
use ct2rs::{Translator, Config};
use ct2rs::tokenizers::auto::Tokenizer;
use crate::error::{AppError, StorageErrorCode};
use super::languages::split_sentences;

pub struct NllbEngine {
    translator: Translator<Tokenizer>,
}

impl NllbEngine {
    pub fn load(model_path: &Path) -> Result<Self, AppError> {
        let translator = Translator::new(model_path, &Config::default()).map_err(|e| {
            AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("load NLLB model: {e}"),
            }
        })?;
        Ok(Self { translator })
    }

    /// Translate each input text as a whole (sentence-split internally, re-joined).
    /// Output vec aligns 1:1 with `texts`.
    ///
    /// CONFIRMED ct2rs API (Task 1 smoke test): sources are PLAIN strings — the
    /// built-in tokenizer encodes them; do NOT pre-tokenize or prepend the source
    /// language token. NLLB auto-detects source; only the target language is passed,
    /// as a target prefix (one Vec<String> per source sentence).
    /// `translate_batch_with_target_prefix` returns Vec<(String, Option<f32>)>.
    pub fn translate(&self, texts: &[String], _src_lang: &str, tgt_lang: &str)
        -> Result<Vec<String>, AppError>
    {
        let mut out = Vec::with_capacity(texts.len());
        for text in texts {
            let sentences = split_sentences(text);
            if sentences.is_empty() {
                out.push(String::new());
                continue;
            }
            // Sources: plain sentence strings (tokenizer handles encoding).
            let sources: Vec<String> = sentences.clone();
            // Target language as a target prefix, one per source sentence.
            let target_prefixes: Vec<Vec<String>> =
                sentences.iter().map(|_| vec![tgt_lang.to_string()]).collect();
            let results = self.translator
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
            let joined = results.into_iter()
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
            out[0].chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c)),
            "translation contains no Bengali characters: {:?}",
            out[0]
        );
        println!("ben: {}", out[0]);
    }
}
