use crate::database::segments::SegmentRow;
use crate::error::AppError;

/// Export segments as RFC 4180 CSV.
///
/// Columns: start_ms, end_ms, start_time, end_time, speaker, text
pub fn export_csv(segments: &[SegmentRow]) -> Result<String, AppError> {
    let mut output = String::new();
    output.push_str("start_ms,end_ms,start_time,end_time,speaker,text\r\n");

    for seg in segments {
        let speaker = seg.speaker_id.as_deref().unwrap_or("");
        output.push_str(&format!(
            "{},{},{},{},{},{}\r\n",
            seg.start_ms,
            seg.end_ms,
            format_timestamp(seg.start_ms),
            format_timestamp(seg.end_ms),
            csv_escape(speaker),
            csv_escape(&seg.text),
        ));
    }

    Ok(output)
}

/// Escape a field per RFC 4180: if the field contains a comma, double-quote,
/// or newline, wrap it in double-quotes and double any internal double-quotes.
fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        let escaped = field.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        field.to_string()
    }
}

fn format_timestamp(ms: i64) -> String {
    let hours = ms / 3_600_000;
    let mins = (ms % 3_600_000) / 60_000;
    let secs = (ms % 60_000) / 1000;
    let millis = ms % 1000;
    format!("{:02}:{:02}:{:02}.{:03}", hours, mins, secs, millis)
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
                end_ms: 10000,
                text: "He said, \"hi there\"".into(),
                speaker_id: None,
                confidence: None,
                is_deleted: false,
            },
        ]
    }

    #[test]
    fn test_export_csv_header() {
        let result = export_csv(&sample_segments()).unwrap();
        assert!(result.starts_with("start_ms,end_ms,start_time,end_time,speaker,text\r\n"));
    }

    #[test]
    fn test_export_csv_row_count() {
        let result = export_csv(&sample_segments()).unwrap();
        let lines: Vec<&str> = result.trim().split("\r\n").collect();
        // header + 2 data rows
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_export_csv_escaping() {
        let result = export_csv(&sample_segments()).unwrap();
        // The text with quotes should be escaped
        assert!(result.contains("\"He said, \"\"hi there\"\"\""));
    }

    #[test]
    fn test_export_csv_empty() {
        let result = export_csv(&[]).unwrap();
        assert_eq!(result, "start_ms,end_ms,start_time,end_time,speaker,text\r\n");
    }

    #[test]
    fn test_csv_escape_plain() {
        assert_eq!(csv_escape("hello"), "hello");
    }

    #[test]
    fn test_csv_escape_with_comma() {
        assert_eq!(csv_escape("hello, world"), "\"hello, world\"");
    }

    #[test]
    fn test_csv_escape_with_quotes() {
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn test_csv_escape_with_newline() {
        assert_eq!(csv_escape("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0), "00:00:00.000");
        assert_eq!(format_timestamp(3723456), "01:02:03.456");
    }
}
