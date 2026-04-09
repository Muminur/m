use crate::database::segments::SegmentRow;
use crate::error::AppError;

/// Export transcript as Markdown.
///
/// Format:
/// ```text
/// ## Title
///
/// **Speaker** `[00:00:00]`
/// text
///
/// ```
pub fn export_markdown(
    title: &str,
    segments: &[SegmentRow],
    duration_ms: i64,
    language: &str,
) -> Result<String, AppError> {
    let mut md = String::new();

    // Title
    md.push_str("## ");
    md.push_str(title);
    md.push_str("\n\n");

    // Metadata
    md.push_str(&format!(
        "**Language:** {} | **Duration:** {} | **Segments:** {}\n\n---\n\n",
        language,
        format_duration(duration_ms),
        segments.len()
    ));

    // Group consecutive segments by speaker
    let mut i = 0;
    while i < segments.len() {
        let seg = &segments[i];
        let speaker = seg.speaker_id.as_deref().unwrap_or("Speaker");
        let timestamp = format_timestamp(seg.start_ms);

        md.push_str(&format!("**{}** `[{}]`\n", speaker, timestamp));

        // Collect consecutive segments from the same speaker
        let mut texts = vec![seg.text.trim().to_string()];
        let mut j = i + 1;
        while j < segments.len() && segments[j].speaker_id.as_deref() == seg.speaker_id.as_deref()
        {
            texts.push(segments[j].text.trim().to_string());
            j += 1;
        }

        md.push_str(&texts.join(" "));
        md.push_str("\n\n");
        i = j;
    }

    Ok(md)
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
                text: "Hello world".into(),
                speaker_id: Some("Alice".into()),
                confidence: Some(0.95),
                is_deleted: false,
            },
            SegmentRow {
                id: "s2".into(),
                transcript_id: "t1".into(),
                index_num: 1,
                start_ms: 5000,
                end_ms: 8000,
                text: "How are you".into(),
                speaker_id: Some("Alice".into()),
                confidence: Some(0.9),
                is_deleted: false,
            },
            SegmentRow {
                id: "s3".into(),
                transcript_id: "t1".into(),
                index_num: 2,
                start_ms: 8000,
                end_ms: 12000,
                text: "I am fine".into(),
                speaker_id: Some("Bob".into()),
                confidence: None,
                is_deleted: false,
            },
        ]
    }

    #[test]
    fn test_export_markdown_title() {
        let result = export_markdown("Test", &sample_segments(), 60000, "en").unwrap();
        assert!(result.starts_with("## Test\n\n"));
    }

    #[test]
    fn test_export_markdown_metadata() {
        let result = export_markdown("Test", &sample_segments(), 60000, "en").unwrap();
        assert!(result.contains("**Language:** en"));
        assert!(result.contains("**Duration:** 1m 0s"));
        assert!(result.contains("**Segments:** 3"));
    }

    #[test]
    fn test_export_markdown_speaker_grouping() {
        let result = export_markdown("Test", &sample_segments(), 60000, "en").unwrap();
        // Alice's consecutive segments should be grouped
        assert!(result.contains("**Alice** `[00:00:00]`\nHello world How are you"));
        // Bob separate
        assert!(result.contains("**Bob** `[00:00:08]`\nI am fine"));
    }

    #[test]
    fn test_export_markdown_empty() {
        let result = export_markdown("Empty", &[], 0, "en").unwrap();
        assert!(result.contains("## Empty"));
        assert!(result.contains("Segments: 0"));
    }

    #[test]
    fn test_export_markdown_no_speaker() {
        let segs = vec![SegmentRow {
            id: "s1".into(),
            transcript_id: "t1".into(),
            index_num: 0,
            start_ms: 0,
            end_ms: 5000,
            text: "Hello".into(),
            speaker_id: None,
            confidence: None,
            is_deleted: false,
        }];
        let result = export_markdown("Test", &segs, 5000, "en").unwrap();
        assert!(result.contains("**Speaker** `[00:00:00]`"));
    }
}
