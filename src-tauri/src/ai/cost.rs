use serde::Serialize;

/// Token and cost estimation for AI operations.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostEstimate {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub estimated_usd: f64,
}

/// Estimate the number of tokens in a text.
///
/// Uses a simple heuristic of approximately 4 characters per token.
/// This is a rough estimate; actual token counts depend on the specific tokenizer.
pub fn estimate_tokens(text: &str) -> u32 {
    let chars = text.len() as f64;
    (chars / 4.0).ceil() as u32
}

/// Estimate the cost of an AI operation based on provider and model pricing.
///
/// Returns a CostEstimate with the estimated USD cost.
/// Uses hardcoded pricing for known models; unknown models return $0.0.
pub fn estimate_cost(
    provider: &str,
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
) -> CostEstimate {
    let (input_cost_per_1k, output_cost_per_1k) = get_pricing(provider, model);

    let estimated_usd = (input_tokens as f64 / 1000.0) * input_cost_per_1k
        + (output_tokens as f64 / 1000.0) * output_cost_per_1k;

    CostEstimate {
        input_tokens,
        output_tokens,
        estimated_usd,
    }
}

/// Get pricing per 1K tokens (input, output) for a given provider/model.
fn get_pricing(provider: &str, model: &str) -> (f64, f64) {
    match (provider, model) {
        // OpenAI
        ("openai", "gpt-4o") => (0.005, 0.015),
        ("openai", "gpt-4o-mini") => (0.00015, 0.0006),

        // Anthropic
        ("anthropic", "claude-opus-4-6") => (0.015, 0.075),
        ("anthropic", "claude-sonnet-4-6") => (0.003, 0.015),
        ("anthropic", "claude-haiku-4-5") => (0.001, 0.005),

        // Groq (near-zero cost)
        ("groq", "llama3-70b-8192") => (0.00059, 0.00079),
        ("groq", "mixtral-8x7b-32768") => (0.00024, 0.00024),

        // Ollama (local, free)
        ("ollama", _) => (0.0, 0.0),

        // Unknown models
        _ => (0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("test"), 1);
        assert_eq!(estimate_tokens("hello world"), 3); // 11 chars / 4 = 2.75, ceil = 3
    }

    #[test]
    fn test_estimate_tokens_longer_text() {
        let text = "a".repeat(400);
        assert_eq!(estimate_tokens(&text), 100);
    }

    #[test]
    fn test_estimate_cost_gpt4o() {
        let est = estimate_cost("openai", "gpt-4o", 1000, 500);
        assert_eq!(est.input_tokens, 1000);
        assert_eq!(est.output_tokens, 500);
        // 1000/1000 * 0.005 + 500/1000 * 0.015 = 0.005 + 0.0075 = 0.0125
        assert!((est.estimated_usd - 0.0125).abs() < 0.0001);
    }

    #[test]
    fn test_estimate_cost_ollama_free() {
        let est = estimate_cost("ollama", "llama3", 10000, 5000);
        assert_eq!(est.estimated_usd, 0.0);
    }

    #[test]
    fn test_estimate_cost_unknown_model() {
        let est = estimate_cost("unknown", "unknown-model", 1000, 500);
        assert_eq!(est.estimated_usd, 0.0);
    }

    #[test]
    fn test_cost_estimate_serializes() {
        let est = CostEstimate {
            input_tokens: 100,
            output_tokens: 50,
            estimated_usd: 0.01,
        };
        let json = serde_json::to_value(&est).unwrap();
        assert_eq!(json["inputTokens"], 100);
        assert_eq!(json["outputTokens"], 50);
        assert_eq!(json["estimatedUsd"], 0.01);
    }
}
