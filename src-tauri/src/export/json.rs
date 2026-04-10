use crate::database::segments::SegmentRow;
use crate::error::{AppError, ExportErrorCode};
use serde::Serialize;

/// A single segment in the JSON export output.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonSegment {
    index: i64,
    start_ms: i64,
    end_ms: i64,
    start_time: String,
    end_time: String,
    speaker: Option<String>,
    text: String,
    confidence: Option<f64>,
}

/// The top-level JSON export document.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonExport {
    title: String,
    duration_ms: i64,
    language: String,
    segment_count: usize,
    segments: Vec<JsonSegment>,
}

/// Export transcript data as structured JSON.
pub fn export_json(
    title: &str,
    segments: &[SegmentRow],
    duration_ms: i64,
    language: &str,
) -> Result<String, AppError> {
    let json_segments: Vec<JsonSegment> = segments
        .iter()
        .map(|seg| JsonSegment {
            index: seg.index_num,
            start_ms: seg.start_ms,
            end_ms: seg.end_ms,
            start_time: format_timestamp(seg.start_ms),
            end_time: format_timestamp(seg.end_ms),
            speaker: seg.speaker_id.clone(),
            text: seg.text.clone(),
            confidence: seg.confidence,
        })
        .collect();

    let export = JsonExport {
        title: title.to_string(),
        duration_ms,
        language: language.to_string(),
        segment_count: json_segments.len(),
        segments: json_segments,
    };

    serde_json::to_string_pretty(&export).map_err(|e| AppError::ExportError {
        code: ExportErrorCode::FormatError,
        message: format!("Failed to serialize JSON: {}", e),
    })
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
                text: "Goodbye".into(),
                speaker_id: None,
                confidence: None,
                is_deleted: false,
            },
        ]
    }

    #[test]
    fn test_export_json_structure() {
        let result = export_json("Test", &sample_segments(), 60000, "en").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["title"], "Test");
        assert_eq!(parsed["durationMs"], 60000);
        assert_eq!(parsed["language"], "en");
        assert_eq!(parsed["segmentCount"], 2);
        assert!(parsed["segments"].is_array());
    }

    #[test]
    fn test_export_json_segment_fields() {
        let result = export_json("Test", &sample_segments(), 60000, "en").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let seg0 = &parsed["segments"][0];
        assert_eq!(seg0["startMs"], 0);
        assert_eq!(seg0["endMs"], 5000);
        assert_eq!(seg0["speaker"], "Alice");
        assert_eq!(seg0["text"], "Hello world");
        assert_eq!(seg0["startTime"], "00:00:00.000");
    }

    #[test]
    fn test_export_json_null_speaker() {
        let result = export_json("Test", &sample_segments(), 60000, "en").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["segments"][1]["speaker"].is_null());
    }

    #[test]
    fn test_export_json_empty() {
        let result = export_json("Empty", &[], 0, "en").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["segmentCount"], 0);
        assert_eq!(parsed["segments"].as_array().unwrap().len(), 0);
    }
}
