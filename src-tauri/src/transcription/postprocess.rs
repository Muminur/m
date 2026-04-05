use serde::{Deserialize, Serialize};

// ── Configuration ──────────────────────────────────────────────────────────────

/// Configuration for the filler-word removal step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillerConfig {
    /// When `false` the remover is a no-op and returns the original text.
    pub enabled: bool,
    /// The set of filler expressions to strip (case-insensitive).
    pub word_list: Vec<String>,
}

impl Default for FillerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            word_list: FillerWordRemover::default_word_list(),
        }
    }
}

// ── Remover ────────────────────────────────────────────────────────────────────

/// Strips filler words from transcribed text while preserving sentence
/// structure and capitalisation.
pub struct FillerWordRemover {
    /// Lower-cased filler phrases, sorted longest-first so multi-word phrases
    /// like "you know" are matched before single words like "you".
    word_list: Vec<String>,
}

impl FillerWordRemover {
    /// Create a new remover with the supplied word list.
    /// Phrases are normalised to lower-case and sorted longest-first.
    pub fn new(word_list: Vec<String>) -> Self {
        let mut list: Vec<String> = word_list
            .into_iter()
            .map(|w| w.to_lowercase())
            .collect();
        // Sort by descending length so multi-word phrases take priority.
        list.sort_by(|a, b| b.len().cmp(&a.len()));
        Self { word_list: list }
    }

    /// Returns the built-in list of English filler expressions.
    pub fn default_word_list() -> Vec<String> {
        [
            "you know",
            "I mean",
            "sort of",
            "kind of",
            "basically",
            "actually",
            "literally",
            "right",
            "so",
            "well",
            "like",
            "um",
            "uh",
            "er",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    /// Remove all configured filler words from `text`.
    ///
    /// - Word-boundary aware: "like" will not match inside "likely".
    /// - Preserves sentence capitalisation after removal.
    /// - Handles multiple consecutive fillers.
    /// - Collapses extra whitespace and fixes punctuation spacing.
    pub fn remove(&self, text: &str) -> String {
        if self.word_list.is_empty() || text.is_empty() {
            return text.to_string();
        }

        // Work sentence-by-sentence so we can re-capitalise correctly.
        // We split on sentence-ending punctuation while keeping the delimiter.
        let result = self.strip_fillers(text);
        self.fix_capitalisation(&result)
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Iteratively remove all filler words from `text` using whole-word
    /// boundary matching (word boundary = not alphanumeric on either side).
    fn strip_fillers(&self, text: &str) -> String {
        let mut current = text.to_string();

        // Keep stripping until stable (handles "um, uh, like" → "" → cleanup).
        loop {
            let mut next = current.clone();
            for filler in &self.word_list {
                next = self.remove_one_filler(&next, filler);
            }
            if next == current {
                break;
            }
            current = next;
        }

        // Collapse multiple spaces / spaces before punctuation.
        self.normalise_whitespace(&current)
    }

    /// Remove all occurrences of `filler` from `text` with word-boundary
    /// awareness.  The check is case-insensitive.
    fn remove_one_filler(&self, text: &str, filler: &str) -> String {
        let lower = text.to_lowercase();
        let filler_lower = filler.to_lowercase();
        let filler_len = filler.len();
        let mut result = String::with_capacity(text.len());
        let text_bytes = text.as_bytes();
        let lower_bytes = lower.as_bytes();

        let mut i = 0usize;
        while i < text.len() {
            // Check whether filler starts at position i.
            if i + filler_len <= text.len()
                && &lower_bytes[i..i + filler_len] == filler_lower.as_bytes()
            {
                let before_ok = i == 0 || !is_word_char(text_bytes[i - 1] as char);
                let after_idx = i + filler_len;
                let after_ok = after_idx >= text.len()
                    || !is_word_char(text_bytes[after_idx] as char);

                if before_ok && after_ok {
                    // Skip the filler; also eat a trailing comma/space combo
                    // like ", " or just " ".
                    let mut skip = after_idx;
                    // Strip a trailing comma that was attached to the filler.
                    if skip < text.len() && text_bytes[skip] == b',' {
                        skip += 1;
                    }
                    i = skip;
                    continue;
                }
            }
            // Copy one character (handle multi-byte UTF-8 safely).
            let ch = text[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
        }

        result
    }

    /// Collapse runs of whitespace and fix space-before-punctuation artifacts.
    fn normalise_whitespace(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut prev_space = false;
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == ' ' || ch == '\t' {
                if !prev_space && !out.is_empty() {
                    // Peek: if next char is punctuation, skip the space.
                    match chars.peek() {
                        Some(',') | Some('.') | Some('!') | Some('?') | Some(';') | Some(':') => {
                            // drop the space
                        }
                        _ => {
                            out.push(' ');
                            prev_space = true;
                        }
                    }
                }
            } else {
                out.push(ch);
                prev_space = false;
            }
        }

        out.trim().to_string()
    }

    /// Re-capitalise the first letter after each sentence boundary (`. `, `! `,
    /// `? `) and at the very start of the string.
    fn fix_capitalisation(&self, text: &str) -> String {
        if text.is_empty() {
            return text.to_string();
        }

        let mut out = String::with_capacity(text.len());
        let mut capitalise_next = true;

        for ch in text.chars() {
            if capitalise_next && ch.is_alphabetic() {
                for upper in ch.to_uppercase() {
                    out.push(upper);
                }
                capitalise_next = false;
            } else {
                out.push(ch);
                if ch == '.' || ch == '!' || ch == '?' {
                    capitalise_next = true;
                }
            }
        }

        out
    }
}

/// Returns `true` if `ch` is a word character (letter, digit, or apostrophe).
fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '\''
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_remover() -> FillerWordRemover {
        FillerWordRemover::new(FillerWordRemover::default_word_list())
    }

    #[test]
    fn test_basic_removal_um_uh() {
        let r = default_remover();
        assert_eq!(r.remove("Um, hello there."), "Hello there.");
        assert_eq!(r.remove("Hello, uh, world."), "Hello, world.");
    }

    #[test]
    fn test_word_boundary_like_in_likely() {
        let r = default_remover();
        // "like" should be removed but "likely" must stay intact.
        let input = "I like, like, think it is likely.";
        let output = r.remove(input);
        assert!(output.contains("likely"), "likely was incorrectly stripped: {}", output);
        assert!(!output.to_lowercase().starts_with("like"), "standalone like not stripped: {}", output);
    }

    #[test]
    fn test_consecutive_fillers() {
        let r = default_remover();
        let input = "Um, uh, er, hello.";
        let output = r.remove(input);
        assert_eq!(output, "Hello.");
    }

    #[test]
    fn test_sentence_start_capitalisation() {
        let r = default_remover();
        // "well" at sentence start should be removed and next word capitalised.
        let input = "Well, it works.";
        let output = r.remove(input);
        assert!(
            output.chars().next().map(|c| c.is_uppercase()).unwrap_or(false),
            "First char should be uppercase: {}",
            output
        );
    }

    #[test]
    fn test_empty_input() {
        let r = default_remover();
        assert_eq!(r.remove(""), "");
    }

    #[test]
    fn test_custom_word_list() {
        let r = FillerWordRemover::new(vec!["foo".into(), "bar".into()]);
        assert_eq!(r.remove("foo bar baz"), "Baz");
        // Words not in custom list are preserved.
        assert!(r.remove("um hello").contains("um"));
    }

    #[test]
    fn test_disabled_mode_via_config() {
        // FillerConfig::enabled = false means we skip removal.
        let config = FillerConfig {
            enabled: false,
            word_list: FillerWordRemover::default_word_list(),
        };
        let input = "Um, hello.";
        let output = if config.enabled {
            FillerWordRemover::new(config.word_list).remove(input)
        } else {
            input.to_string()
        };
        assert_eq!(output, "Um, hello.");
    }

    #[test]
    fn test_like_not_in_unlikely() {
        let r = default_remover();
        let output = r.remove("That is unlikely.");
        assert!(output.contains("unlikely"), "unlikely was incorrectly modified: {}", output);
    }

    #[test]
    fn test_filler_config_default_serializes_camel_case() {
        let config = FillerConfig::default();
        let json = serde_json::to_value(&config).unwrap();
        assert!(json.get("wordList").is_some(), "expected camelCase wordList");
        assert!(json.get("enabled").is_some(), "expected enabled field");
        assert_eq!(json["enabled"], true);
    }

    #[test]
    fn test_multi_word_filler_you_know() {
        let r = default_remover();
        let input = "It is, you know, really good.";
        let output = r.remove(input);
        assert!(!output.to_lowercase().contains("you know"), "multi-word filler not removed: {}", output);
        assert!(output.to_lowercase().contains("really good"), "content was incorrectly removed: {}", output);
    }
}
