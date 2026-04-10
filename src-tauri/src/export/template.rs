use crate::database::segments::SegmentRow;
use crate::error::{AppError, ExportErrorCode};
use handlebars::Handlebars;
use serde::Serialize;

/// A segment exposed to Handlebars templates.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TemplateSegment {
    index: i64,
    start_ms: i64,
    end_ms: i64,
    start_time: String,
    end_time: String,
    speaker: String,
    text: String,
    confidence: Option<f64>,
}

/// The full data context passed to Handlebars.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TemplateContext {
    title: String,
    duration_ms: i64,
    language: String,
    segment_count: usize,
    speakers: Vec<String>,
    segments: Vec<TemplateSegment>,
}

/// Render an export using a user-supplied Handlebars template string.
///
/// Available variables:
/// - `{{title}}`, `{{durationMs}}`, `{{language}}`, `{{segmentCount}}`, `{{speakers}}`
/// - `{{#each segments}}` with: `{{startMs}}`, `{{endMs}}`, `{{startTime}}`,
///   `{{endTime}}`, `{{speaker}}`, `{{text}}`, `{{confidence}}`
pub fn render_export_template(
    template_str: &str,
    title: &str,
    segments: &[SegmentRow],
    duration_ms: i64,
    language: &str,
) -> Result<String, AppError> {
    let mut hbs = Handlebars::new();
    hbs.set_strict_mode(true);

    hbs.register_template_string("export", template_str)
        .map_err(|e| AppError::ExportError {
            code: ExportErrorCode::TemplateError,
            message: format!("Invalid template: {}", e),
        })?;

    let template_segments: Vec<TemplateSegment> = segments
        .iter()
        .map(|seg| TemplateSegment {
            index: seg.index_num,
            start_ms: seg.start_ms,
            end_ms: seg.end_ms,
            start_time: format_timestamp(seg.start_ms),
            end_time: format_timestamp(seg.end_ms),
            speaker: seg.speaker_id.clone().unwrap_or_default(),
            text: seg.text.clone(),
            confidence: seg.confidence,
        })
        .collect();

    // Extract unique non-empty speaker names in insertion order
    let mut speakers: Vec<String> = Vec::new();
    for seg in segments {
        if let Some(ref spk) = seg.speaker_id {
            if !spk.is_empty() && !speakers.contains(spk) {
                speakers.push(spk.clone());
            }
        }
    }

    let context = TemplateContext {
        title: title.to_string(),
        duration_ms,
        language: language.to_string(),
        segment_count: template_segments.len(),
        speakers,
        segments: template_segments,
    };

    hbs.render("export", &context)
        .map_err(|e| AppError::ExportError {
            code: ExportErrorCode::TemplateError,
            message: format!("Template render failed: {}", e),
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
    fn test_render_simple_template() {
        let template = "Title: {{title}}\n{{#each segments}}{{startTime}}: {{text}}\n{{/each}}";
        let result =
            render_export_template(template, "Test", &sample_segments(), 60000, "en").unwrap();
        assert!(result.contains("Title: Test"));
        assert!(result.contains("00:00:00.000: Hello world"));
        assert!(result.contains("00:00:05.000: Goodbye"));
    }

    #[test]
    fn test_render_template_metadata() {
        let template = "{{title}} ({{language}}) - {{durationMs}}ms - {{segmentCount}} segments";
        let result =
            render_export_template(template, "My Talk", &sample_segments(), 120000, "fr").unwrap();
        assert_eq!(result, "My Talk (fr) - 120000ms - 2 segments");
    }

    #[test]
    fn test_render_template_speaker() {
        let template = "{{#each segments}}{{speaker}}: {{text}}\n{{/each}}";
        let result =
            render_export_template(template, "T", &sample_segments(), 0, "en").unwrap();
        assert!(result.contains("Alice: Hello world"));
        // No speaker produces empty string
        assert!(result.contains(": Goodbye"));
    }

    #[test]
    fn test_render_invalid_template() {
        let template = "{{#each broken";
        let result = render_export_template(template, "T", &[], 0, "en");
        assert!(result.is_err());
    }

    #[test]
    fn test_render_empty_segments() {
        let template = "Count: {{segmentCount}}";
        let result = render_export_template(template, "T", &[], 0, "en").unwrap();
        assert_eq!(result, "Count: 0");
    }
}
