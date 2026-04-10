use crate::database::segments::SegmentRow;
use crate::error::{AppError, IntegrationErrorCode};
use crate::network::guard::NetworkGuard;
use serde_json::json;

/// Configuration for Notion integration.
pub struct NotionConfig {
    pub api_key: String,
    pub database_id: String,
}

/// Format milliseconds as HH:MM:SS.
fn format_duration(ms: i64) -> String {
    let total_secs = ms / 1000;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

/// Format a segment timestamp as [HH:MM:SS].
fn format_timestamp(ms: i64) -> String {
    format!("[{}]", format_duration(ms))
}

/// Build Notion paragraph blocks from transcript segments.
///
/// Notion API limits children to 100 blocks per request, so we truncate
/// if the segment count exceeds that limit.
fn build_paragraph_blocks(segments: &[SegmentRow]) -> Vec<serde_json::Value> {
    let max_blocks = 100;
    let capped = if segments.len() > max_blocks {
        &segments[..max_blocks]
    } else {
        segments
    };

    capped
        .iter()
        .map(|seg| {
            let timestamp = format_timestamp(seg.start_ms);
            let speaker_prefix = seg
                .speaker_id
                .as_deref()
                .map(|s| format!("{}: ", s))
                .unwrap_or_default();
            let content = format!("{} {}{}", timestamp, speaker_prefix, seg.text.trim());

            json!({
                "object": "block",
                "type": "paragraph",
                "paragraph": {
                    "rich_text": [{
                        "type": "text",
                        "text": { "content": content }
                    }]
                }
            })
        })
        .collect()
}

/// Push a transcript to a Notion database as a new page.
///
/// Returns the URL of the created Notion page.
pub async fn push_to_notion(
    config: &NotionConfig,
    title: &str,
    segments: &[SegmentRow],
    duration_ms: i64,
    language: &str,
    guard: &NetworkGuard,
) -> Result<String, AppError> {
    let children = build_paragraph_blocks(segments);
    let duration_str = format_duration(duration_ms);

    let body = json!({
        "parent": { "database_id": &config.database_id },
        "properties": {
            "Name": {
                "title": [{
                    "text": { "content": title }
                }]
            },
            "Language": {
                "rich_text": [{
                    "text": { "content": language }
                }]
            },
            "Duration": {
                "rich_text": [{
                    "text": { "content": duration_str }
                }]
            }
        },
        "children": children
    });

    let req = guard
        .client()
        .post("https://api.notion.com/v1/pages")
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Notion-Version", "2022-06-28")
        .header("Content-Type", "application/json")
        .json(&body);

    let resp = guard.request(req).await?;
    let status = resp.status();

    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::AuthenticationFailed,
            message: "Notion API key is invalid or expired".into(),
        });
    }

    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::RateLimited,
            message: "Notion API rate limit exceeded".into(),
        });
    }

    if !status.is_success() {
        let error_text = resp.text().await.unwrap_or_default();
        return Err(AppError::IntegrationError {
            code: IntegrationErrorCode::ApiError,
            message: format!("Notion API error ({}): {}", status, error_text),
        });
    }

    let result: serde_json::Value = resp.json().await.map_err(|e| AppError::IntegrationError {
        code: IntegrationErrorCode::ApiError,
        message: format!("Failed to parse Notion response: {}", e),
    })?;

    let page_url = result["url"].as_str().unwrap_or("").to_string();

    if page_url.is_empty() {
        // Construct URL from page ID as fallback
        let page_id = result["id"].as_str().unwrap_or("");
        if page_id.is_empty() {
            return Err(AppError::IntegrationError {
                code: IntegrationErrorCode::ApiError,
                message: "Notion response missing page URL and ID".into(),
            });
        }
        Ok(format!(
            "https://www.notion.so/{}",
            page_id.replace('-', "")
        ))
    } else {
        Ok(page_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "00:00:00");
        assert_eq!(format_duration(61_000), "00:01:01");
        assert_eq!(format_duration(3_661_000), "01:01:01");
    }

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(5_000), "[00:00:05]");
    }

    #[test]
    fn test_build_paragraph_blocks_empty() {
        let blocks = build_paragraph_blocks(&[]);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_build_paragraph_blocks_caps_at_100() {
        let segments: Vec<SegmentRow> = (0..150)
            .map(|i| SegmentRow {
                id: format!("seg-{}", i),
                transcript_id: "t1".into(),
                index_num: i,
                start_ms: i * 1000,
                end_ms: (i + 1) * 1000,
                text: format!("Segment {}", i),
                speaker_id: None,
                confidence: Some(0.9),
                is_deleted: false,
            })
            .collect();
        let blocks = build_paragraph_blocks(&segments);
        assert_eq!(blocks.len(), 100);
    }

    #[test]
    fn test_build_paragraph_blocks_includes_speaker() {
        let seg = SegmentRow {
            id: "s1".into(),
            transcript_id: "t1".into(),
            index_num: 0,
            start_ms: 0,
            end_ms: 1000,
            text: "Hello world".into(),
            speaker_id: Some("Alice".into()),
            confidence: None,
            is_deleted: false,
        };
        let blocks = build_paragraph_blocks(&[seg]);
        let content = blocks[0]["paragraph"]["rich_text"][0]["text"]["content"]
            .as_str()
            .unwrap();
        assert!(content.contains("Alice:"));
        assert!(content.contains("Hello world"));
    }
}
