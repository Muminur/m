use crate::database::segments::SegmentRow;
use crate::error::AppError;

/// Export transcript as styled HTML with inline CSS.
///
/// Produces a complete HTML document with timestamp spans and speaker label spans.
pub fn export_html(
    title: &str,
    segments: &[SegmentRow],
    duration_ms: i64,
    language: &str,
) -> Result<String, AppError> {
    let mut html = String::new();

    html.push_str("<!DOCTYPE html>\n<html lang=\"");
    html.push_str(&html_escape(language));
    html.push_str("\">\n<head>\n<meta charset=\"UTF-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str("<title>");
    html.push_str(&html_escape(title));
    html.push_str("</title>\n");
    html.push_str("<style>\n");
    html.push_str(INLINE_CSS);
    html.push_str("</style>\n</head>\n<body>\n");

    // Header
    html.push_str("<div class=\"transcript\">\n");
    html.push_str("<h1>");
    html.push_str(&html_escape(title));
    html.push_str("</h1>\n");

    // Metadata
    html.push_str("<div class=\"meta\">\n");
    html.push_str("<span class=\"meta-item\">Language: ");
    html.push_str(&html_escape(language));
    html.push_str("</span>\n");
    html.push_str("<span class=\"meta-item\">Duration: ");
    html.push_str(&format_duration(duration_ms));
    html.push_str("</span>\n");
    html.push_str("<span class=\"meta-item\">Segments: ");
    html.push_str(&segments.len().to_string());
    html.push_str("</span>\n");
    html.push_str("</div>\n");

    // Segments
    html.push_str("<div class=\"segments\">\n");
    for seg in segments {
        html.push_str("<div class=\"segment\">\n");

        // Timestamp
        html.push_str("<span class=\"timestamp\">[");
        html.push_str(&format_timestamp(seg.start_ms));
        html.push_str("]</span>\n");

        // Speaker
        if let Some(ref speaker) = seg.speaker_id {
            let color_class = speaker_color_class(speaker);
            html.push_str("<span class=\"speaker ");
            html.push_str(&color_class);
            html.push_str("\">");
            html.push_str(&html_escape(speaker));
            html.push_str(":</span>\n");
        }

        // Text
        html.push_str("<span class=\"text\">");
        html.push_str(&html_escape(&seg.text));
        html.push_str("</span>\n");

        html.push_str("</div>\n");
    }
    html.push_str("</div>\n");
    html.push_str("</div>\n");
    html.push_str("</body>\n</html>");

    Ok(html)
}

const INLINE_CSS: &str = r#"
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  max-width: 800px;
  margin: 40px auto;
  padding: 0 20px;
  color: #333;
  line-height: 1.6;
  background: #fafafa;
}
h1 { color: #1a1a1a; margin-bottom: 8px; }
.meta {
  color: #666;
  font-size: 0.9em;
  margin-bottom: 24px;
  padding-bottom: 16px;
  border-bottom: 1px solid #ddd;
}
.meta-item { margin-right: 20px; }
.segment {
  margin-bottom: 12px;
  padding: 8px 12px;
  border-radius: 4px;
  background: #fff;
  border-left: 3px solid #ddd;
}
.timestamp {
  font-family: "SF Mono", "Fira Code", monospace;
  color: #888;
  font-size: 0.85em;
  margin-right: 8px;
}
.speaker {
  font-weight: 600;
  margin-right: 6px;
}
.text { }
.speaker-color-0 { color: #2563eb; border-left-color: #2563eb; }
.speaker-color-1 { color: #dc2626; border-left-color: #dc2626; }
.speaker-color-2 { color: #059669; border-left-color: #059669; }
.speaker-color-3 { color: #7c3aed; border-left-color: #7c3aed; }
.speaker-color-4 { color: #d97706; border-left-color: #d97706; }
.speaker-color-5 { color: #0891b2; border-left-color: #0891b2; }
.speaker-color-6 { color: #be185d; border-left-color: #be185d; }
.speaker-color-7 { color: #4f46e5; border-left-color: #4f46e5; }
"#;

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn format_timestamp(ms: i64) -> String {
    let hours = ms / 3_600_000;
    let mins = (ms % 3_600_000) / 60_000;
    let secs = (ms % 60_000) / 1000;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}

fn format_duration(ms: i64) -> String {
    let hours = ms / 3_600_000;
    let mins = (ms % 3_600_000) / 60_000;
    let secs = (ms % 60_000) / 1000;
    if hours > 0 {
        format!("{}h {}m {}s", hours, mins, secs)
    } else {
        format!("{}m {}s", mins, secs)
    }
}

/// Assign a stable color class based on the speaker name hash.
fn speaker_color_class(speaker: &str) -> String {
    let hash: u32 = speaker.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    format!("speaker-color-{}", hash % 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_segments() -> Vec<SegmentRow> {
        vec![
            SegmentRow {
                id: "s1".into(),
                transcript_id: "t1".into(),
                index_num: 0,
                start_ms: 0,
                end_ms: 5000,
                text: "Hello <world> & \"friends\"".into(),
                speaker_id: Some("Alice".into()),
                confidence: Some(0.95),
                is_deleted: false,
            },
            SegmentRow {
                id: "s2".into(),
                transcript_id: "t1".into(),
                index_num: 1,
                start_ms: 5000,
                end_ms: 10000,
                text: "Goodbye".into(),
                speaker_id: Some("Bob".into()),
                confidence: None,
                is_deleted: false,
            },
        ]
    }

    #[test]
    fn test_export_html_structure() {
        let result = export_html("Test", &sample_segments(), 60000, "en").unwrap();
        assert!(result.starts_with("<!DOCTYPE html>"));
        assert!(result.contains("<title>Test</title>"));
        assert!(result.contains("</html>"));
    }

    #[test]
    fn test_export_html_escaping() {
        let result = export_html("Test", &sample_segments(), 60000, "en").unwrap();
        assert!(result.contains("Hello &lt;world&gt; &amp; &quot;friends&quot;"));
        assert!(!result.contains("Hello <world>"));
    }

    #[test]
    fn test_export_html_speaker_colors() {
        let result = export_html("Test", &sample_segments(), 60000, "en").unwrap();
        assert!(result.contains("speaker-color-"));
    }

    #[test]
    fn test_export_html_timestamps() {
        let result = export_html("Test", &sample_segments(), 60000, "en").unwrap();
        assert!(result.contains("[00:00:00]"));
        assert!(result.contains("[00:00:05]"));
    }

    #[test]
    fn test_export_html_empty() {
        let result = export_html("Empty", &[], 0, "en").unwrap();
        assert!(result.contains("<h1>Empty</h1>"));
        assert!(result.contains("Segments: 0"));
    }

    #[test]
    fn test_html_escape_all_chars() {
        assert_eq!(html_escape("a&b<c>d\"e'f"), "a&amp;b&lt;c&gt;d&quot;e&#39;f");
    }

    #[test]
    fn test_speaker_color_class_deterministic() {
        let c1 = speaker_color_class("Alice");
        let c2 = speaker_color_class("Alice");
        assert_eq!(c1, c2);
    }
}
