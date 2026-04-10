use crate::error::{AppError, IntegrationErrorCode};
use crate::network::guard::NetworkGuard;
use serde::Deserialize;

/// Maximum number of text items per DeepL API request.
const DEEPL_BATCH_LIMIT: usize = 50;

/// Configuration for DeepL translation.
pub struct DeepLConfig {
    pub api_key: String,
    pub target_lang: String,
}

/// A single translation result from DeepL.
#[derive(Deserialize)]
struct DeepLTranslation {
    text: String,
}

/// DeepL API response.
#[derive(Deserialize)]
struct DeepLResponse {
    translations: Vec<DeepLTranslation>,
}

/// Select the correct DeepL API endpoint based on the API key.
fn deepl_endpoint(api_key: &str) -> &'static str {
    if api_key.ends_with(":fx") {
        "https://api-free.deepl.com/v2/translate"
    } else {
        "https://api.deepl.com/v2/translate"
    }
}

/// Check a DeepL HTTP response status and return a typed error on failure.
fn check_deepl_response_status(
    status: reqwest::StatusCode,
    error_text: &str,
) -> Result<(), AppError> {
    if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::AuthenticationFailed,
            message: "DeepL API key is invalid".into(),
        });
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::RateLimited,
            message: "DeepL API rate limit exceeded".into(),
        });
    }
    if !status.is_success() {
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::ApiError,
            message: format!("DeepL API error ({}): {}", status, error_text),
        });
    }
    Ok(())
}

/// Translate a block of text using the DeepL API.
pub async fn translate_text(
    config: &DeepLConfig,
    text: &str,
    guard: &NetworkGuard,
) -> Result<String, AppError> {
    if text.is_empty() {
        return Ok(String::new());
    }

    let base_url = deepl_endpoint(&config.api_key);

    let req = guard
        .client()
        .post(base_url)
        .header(
            "Authorization",
            format!("DeepL-Auth-Key {}", config.api_key),
        )
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "text": [text],
            "target_lang": config.target_lang.to_uppercase(),
        }));

    let resp = guard.request(req).await?;
    let status = resp.status();

    if !status.is_success() {
        let error_text = resp.text().await.unwrap_or_default();
        check_deepl_response_status(status, &error_text)?;
        // check_deepl_response_status always returns Err for non-success, but
        // the compiler cannot prove that, so we return explicitly.
        unreachable!();
    }

    let result: DeepLResponse = resp.json().await.map_err(|e| AppError::IntegrationError {
        code: IntegrationErrorCode::ApiError,
        message: format!("Failed to parse DeepL response: {}", e),
    })?;

    result
        .translations
        .into_iter()
        .next()
        .map(|t| t.text)
        .ok_or_else(|| AppError::IntegrationError {
            code: IntegrationErrorCode::ApiError,
            message: "DeepL returned no translations".into(),
        })
}

/// Translate SRT content line-by-line, preserving SRT structure.
///
/// Only translates the text lines; sequence numbers, timestamps,
/// and blank lines are preserved as-is. Uses the array batch API
/// so each text line gets its own translation (no newline-splitting risk).
pub async fn translate_srt(
    config: &DeepLConfig,
    srt_content: &str,
    guard: &NetworkGuard,
) -> Result<String, AppError> {
    if srt_content.is_empty() {
        return Ok(String::new());
    }

    // Collect translatable text lines from SRT
    let mut text_lines: Vec<String> = Vec::new();
    let mut line_indices: Vec<usize> = Vec::new();
    let lines: Vec<&str> = srt_content.lines().collect();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();

        // Skip sequence numbers (pure digits)
        if line.chars().all(|c| c.is_ascii_digit()) && !line.is_empty() {
            i += 1;
            continue;
        }

        // Skip timestamp lines (contain " --> ")
        if line.contains(" --> ") {
            i += 1;
            continue;
        }

        // Skip blank lines
        if line.is_empty() {
            i += 1;
            continue;
        }

        // This is a text line - collect for batch translation
        text_lines.push(line.to_string());
        line_indices.push(i);
        i += 1;
    }

    if text_lines.is_empty() {
        return Ok(srt_content.to_string());
    }

    // Translate all text lines using array batch API (chunked by DEEPL_BATCH_LIMIT)
    let mut all_translations: Vec<String> = Vec::with_capacity(text_lines.len());
    let base_url = deepl_endpoint(&config.api_key);

    for chunk in text_lines.chunks(DEEPL_BATCH_LIMIT) {
        let batch: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();

        let req = guard
            .client()
            .post(base_url)
            .header(
                "Authorization",
                format!("DeepL-Auth-Key {}", config.api_key),
            )
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "text": batch,
                "target_lang": config.target_lang.to_uppercase(),
            }));

        let resp = guard.request(req).await?;
        let status = resp.status();

        if !status.is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            check_deepl_response_status(status, &error_text)?;
            unreachable!();
        }

        let result: DeepLResponse = resp.json().await.map_err(|e| AppError::IntegrationError {
            code: IntegrationErrorCode::ApiError,
            message: format!("Failed to parse DeepL response: {}", e),
        })?;

        for t in result.translations {
            all_translations.push(t.text);
        }
    }

    // Rebuild SRT with translated text
    let mut result_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

    for (idx, line_idx) in line_indices.iter().enumerate() {
        if let Some(translated_line) = all_translations.get(idx) {
            result_lines[*line_idx] = translated_line.clone();
        }
    }

    Ok(result_lines.join("\n"))
}

/// Translate each segment's text individually using the DeepL API.
///
/// Returns a Vec of translated text strings, one per input segment.
/// Segments with empty text are returned as empty strings without an API call.
/// Requests are chunked to respect the DeepL 50-text-per-request limit.
pub async fn translate_segments(
    config: &DeepLConfig,
    texts: &[String],
    guard: &NetworkGuard,
) -> Result<Vec<String>, AppError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }

    // Filter out empty texts, track positions
    let non_empty: Vec<(usize, &str)> = texts
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.is_empty())
        .map(|(i, t)| (i, t.as_str()))
        .collect();

    if non_empty.is_empty() {
        return Ok(texts.iter().map(|_| String::new()).collect());
    }

    let base_url = deepl_endpoint(&config.api_key);

    // Translate in chunks of DEEPL_BATCH_LIMIT
    let mut all_translations: Vec<DeepLTranslation> = Vec::with_capacity(non_empty.len());

    for chunk in non_empty.chunks(DEEPL_BATCH_LIMIT) {
        let batch: Vec<&str> = chunk.iter().map(|(_, t)| *t).collect();

        let req = guard
            .client()
            .post(base_url)
            .header(
                "Authorization",
                format!("DeepL-Auth-Key {}", config.api_key),
            )
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "text": batch,
                "target_lang": config.target_lang.to_uppercase(),
            }));

        let resp = guard.request(req).await?;
        let status = resp.status();

        if !status.is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            check_deepl_response_status(status, &error_text)?;
            unreachable!();
        }

        let result: DeepLResponse = resp.json().await.map_err(|e| AppError::IntegrationError {
            code: IntegrationErrorCode::ApiError,
            message: format!("Failed to parse DeepL response: {}", e),
        })?;

        all_translations.extend(result.translations);
    }

    // Re-map translated texts back to original positions
    let mut output: Vec<String> = texts.iter().map(|_| String::new()).collect();
    for ((orig_idx, _), translation) in non_empty.iter().zip(all_translations.iter()) {
        output[*orig_idx] = translation.text.clone();
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srt_line_parsing() {
        // Verify the parsing logic identifies text lines correctly
        let srt = "1\n00:00:00,000 --> 00:00:05,000\nHello world\n\n2\n00:00:05,000 --> 00:00:10,000\nGoodbye\n";
        let lines: Vec<&str> = srt.lines().collect();

        let mut text_lines = Vec::new();
        for line in &lines {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.chars().all(|c| c.is_ascii_digit())
                || trimmed.contains(" --> ")
            {
                continue;
            }
            text_lines.push(trimmed);
        }

        assert_eq!(text_lines, vec!["Hello world", "Goodbye"]);
    }

    #[test]
    fn test_deepl_endpoint_free() {
        assert_eq!(
            deepl_endpoint("abc123:fx"),
            "https://api-free.deepl.com/v2/translate"
        );
    }

    #[test]
    fn test_deepl_endpoint_pro() {
        assert_eq!(
            deepl_endpoint("abc123"),
            "https://api.deepl.com/v2/translate"
        );
    }

    #[test]
    fn test_check_response_status_success() {
        assert!(check_deepl_response_status(reqwest::StatusCode::OK, "").is_ok());
    }

    #[test]
    fn test_check_response_status_forbidden() {
        let err = check_deepl_response_status(reqwest::StatusCode::FORBIDDEN, "").unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("invalid"));
    }

    #[test]
    fn test_check_response_status_rate_limited() {
        let err =
            check_deepl_response_status(reqwest::StatusCode::TOO_MANY_REQUESTS, "").unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("rate limit"));
    }
}
