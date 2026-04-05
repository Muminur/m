/// Post-processor for dictated text: replaces spoken punctuation commands
/// with their symbolic equivalents and applies auto-capitalization.
///
/// A single punctuation command mapping.
#[derive(Debug, Clone)]
struct PunctuationCommand {
    /// The spoken phrase (lowercase).
    phrase: &'static str,
    /// The replacement text.
    replacement: &'static str,
    /// Whether the next character should be capitalized.
    capitalize_next: bool,
}

/// Post-processor with configurable punctuation commands.
pub struct PostProcessor {
    commands: Vec<PunctuationCommand>,
}

impl PostProcessor {
    /// Create a new post-processor with default punctuation commands.
    pub fn new() -> Self {
        Self {
            commands: Self::default_commands(),
        }
    }

    fn default_commands() -> Vec<PunctuationCommand> {
        vec![
            PunctuationCommand {
                phrase: "period",
                replacement: ".",
                capitalize_next: true,
            },
            PunctuationCommand {
                phrase: "full stop",
                replacement: ".",
                capitalize_next: true,
            },
            PunctuationCommand {
                phrase: "comma",
                replacement: ",",
                capitalize_next: false,
            },
            PunctuationCommand {
                phrase: "question mark",
                replacement: "?",
                capitalize_next: true,
            },
            PunctuationCommand {
                phrase: "exclamation mark",
                replacement: "!",
                capitalize_next: true,
            },
            PunctuationCommand {
                phrase: "exclamation point",
                replacement: "!",
                capitalize_next: true,
            },
            PunctuationCommand {
                phrase: "new line",
                replacement: "\n",
                capitalize_next: true,
            },
            PunctuationCommand {
                phrase: "new paragraph",
                replacement: "\n\n",
                capitalize_next: true,
            },
            PunctuationCommand {
                phrase: "tab",
                replacement: "\t",
                capitalize_next: false,
            },
            PunctuationCommand {
                phrase: "colon",
                replacement: ":",
                capitalize_next: false,
            },
            PunctuationCommand {
                phrase: "semicolon",
                replacement: ";",
                capitalize_next: false,
            },
            PunctuationCommand {
                phrase: "open quote",
                replacement: "\"",
                capitalize_next: false,
            },
            PunctuationCommand {
                phrase: "close quote",
                replacement: "\"",
                capitalize_next: false,
            },
            PunctuationCommand {
                phrase: "dash",
                replacement: "-",
                capitalize_next: false,
            },
            PunctuationCommand {
                phrase: "hyphen",
                replacement: "-",
                capitalize_next: false,
            },
        ]
    }

    /// Process dictated text: replace punctuation commands and auto-capitalize.
    pub fn process(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }

        // Work on a mutable copy, case-insensitive matching
        let mut result = text.to_string();

        // Sort commands by phrase length descending so longer phrases match first
        let mut sorted_commands: Vec<&PunctuationCommand> = self.commands.iter().collect();
        sorted_commands.sort_by(|a, b| b.phrase.len().cmp(&a.phrase.len()));

        // Replace punctuation commands (case-insensitive)
        // We track which positions need capitalize-next
        let mut capitalize_positions: Vec<usize> = Vec::new();

        for cmd in &sorted_commands {
            let lower = result.to_lowercase();
            let mut search_from = 0;

            while let Some(pos) = lower[search_from..].find(cmd.phrase) {
                {
                    let abs_pos = search_from + pos;
                    let end_pos = abs_pos + cmd.phrase.len();

                    // Check word boundaries: before must be start-of-string or whitespace,
                    // after must be end-of-string or whitespace
                    let before_ok =
                        abs_pos == 0 || result.as_bytes().get(abs_pos - 1) == Some(&b' ');
                    let after_ok =
                        end_pos >= result.len() || result.as_bytes().get(end_pos) == Some(&b' ');

                    if before_ok && after_ok {
                        // Remove leading space before punctuation if present
                        let replace_start =
                            if abs_pos > 0 && result.as_bytes().get(abs_pos - 1) == Some(&b' ') {
                                abs_pos - 1
                            } else {
                                abs_pos
                            };

                        // Remove trailing space after punctuation if present
                        let replace_end = if end_pos < result.len()
                            && result.as_bytes().get(end_pos) == Some(&b' ')
                        {
                            end_pos + 1
                        } else {
                            end_pos
                        };

                        let replacement = if cmd.capitalize_next {
                            format!("{} ", cmd.replacement)
                        } else {
                            cmd.replacement.to_string()
                        };

                        if cmd.capitalize_next {
                            // The character right after the replacement (including the space) should be capitalized
                            capitalize_positions.push(replace_start + replacement.len());
                        }

                        result.replace_range(replace_start..replace_end, &replacement);

                        // Re-run from the same position since string length changed
                        // but we need to recompute lower
                        break; // break inner while, outer for-loop will re-process
                    } else {
                        search_from = abs_pos + 1;
                    }
                }
            }
        }

        // Auto-capitalize after sentence-ending punctuation
        Self::auto_capitalize(&mut result);

        // Capitalize first character
        Self::capitalize_first(&mut result);

        result
    }

    /// Capitalize the character following sentence-ending punctuation (. ? !).
    fn auto_capitalize(text: &mut String) {
        let mut result = String::with_capacity(text.len());
        let mut capitalize_next = false;
        for ch in text.chars() {
            if matches!(ch, '.' | '?' | '!') {
                capitalize_next = true;
                result.push(ch);
            } else if capitalize_next && ch.is_ascii_lowercase() {
                result.push(ch.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                if !ch.is_whitespace() {
                    capitalize_next = false;
                }
                result.push(ch);
            }
        }
        *text = result;
    }

    /// Capitalize the first alphabetic character in the text.
    fn capitalize_first(text: &mut String) {
        let mut result = String::with_capacity(text.len());
        let mut found = false;
        for ch in text.chars() {
            if !found && ch.is_ascii_lowercase() {
                result.push(ch.to_ascii_uppercase());
                found = true;
            } else {
                result.push(ch);
            }
        }
        *text = result;
    }
}

impl Default for PostProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let pp = PostProcessor::new();
        assert_eq!(pp.process(""), "");
    }

    #[test]
    fn test_no_commands_in_text() {
        let pp = PostProcessor::new();
        assert_eq!(pp.process("hello world"), "Hello world");
    }

    #[test]
    fn test_period_replacement() {
        let pp = PostProcessor::new();
        let result = pp.process("hello world period");
        assert_eq!(result, "Hello world.");
    }

    #[test]
    fn test_comma_replacement() {
        let pp = PostProcessor::new();
        let result = pp.process("hello comma world");
        assert_eq!(result, "Hello,world");
    }

    #[test]
    fn test_question_mark_replacement() {
        let pp = PostProcessor::new();
        let result = pp.process("how are you question mark");
        assert_eq!(result, "How are you?");
    }

    #[test]
    fn test_exclamation_mark_replacement() {
        let pp = PostProcessor::new();
        let result = pp.process("wow exclamation mark");
        assert_eq!(result, "Wow!");
    }

    #[test]
    fn test_exclamation_point_replacement() {
        let pp = PostProcessor::new();
        let result = pp.process("wow exclamation point");
        assert_eq!(result, "Wow!");
    }

    #[test]
    fn test_new_line_replacement() {
        let pp = PostProcessor::new();
        let result = pp.process("hello new line world");
        assert_eq!(result, "Hello\n World");
    }

    #[test]
    fn test_new_paragraph_replacement() {
        let pp = PostProcessor::new();
        let result = pp.process("hello new paragraph world");
        assert_eq!(result, "Hello\n\n World");
    }

    #[test]
    fn test_colon_replacement() {
        let pp = PostProcessor::new();
        let result = pp.process("note colon important");
        assert_eq!(result, "Note:important");
    }

    #[test]
    fn test_semicolon_replacement() {
        let pp = PostProcessor::new();
        let result = pp.process("first semicolon second");
        assert_eq!(result, "First;second");
    }

    #[test]
    fn test_dash_replacement() {
        let pp = PostProcessor::new();
        let result = pp.process("well dash known");
        assert_eq!(result, "Well-known");
    }

    #[test]
    fn test_auto_capitalize_after_period() {
        let pp = PostProcessor::new();
        let result = pp.process("hello period world");
        assert_eq!(result, "Hello. World");
    }

    #[test]
    fn test_auto_capitalize_after_question_mark() {
        let pp = PostProcessor::new();
        let result = pp.process("really question mark yes");
        assert_eq!(result, "Really? Yes");
    }

    #[test]
    fn test_auto_capitalize_after_exclamation() {
        let pp = PostProcessor::new();
        let result = pp.process("wow exclamation mark great");
        assert_eq!(result, "Wow! Great");
    }

    #[test]
    fn test_capitalize_first_character() {
        let pp = PostProcessor::new();
        assert_eq!(pp.process("hello"), "Hello");
    }

    #[test]
    fn test_case_insensitive_commands() {
        let pp = PostProcessor::new();
        let result = pp.process("hello Period world");
        assert_eq!(result, "Hello. World");
    }

    #[test]
    fn test_full_stop_alias() {
        let pp = PostProcessor::new();
        let result = pp.process("hello full stop world");
        assert_eq!(result, "Hello. World");
    }

    #[test]
    fn test_open_close_quotes() {
        let pp = PostProcessor::new();
        let result = pp.process("he said open quote hello close quote");
        assert_eq!(result, "He said\"hello\"");
    }

    #[test]
    fn test_tab_replacement() {
        let pp = PostProcessor::new();
        let result = pp.process("column one tab column two");
        assert_eq!(result, "Column one\tcolumn two");
    }

    #[test]
    fn test_multiple_sentences() {
        let pp = PostProcessor::new();
        let result =
            pp.process("hello period how are you question mark i am fine exclamation mark");
        assert_eq!(result, "Hello. How are you? I am fine!");
    }
}
