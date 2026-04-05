use crate::database::{transcripts::TranscriptRow, segments::SegmentRow};

pub fn render(transcript: &TranscriptRow, segments: &[SegmentRow], include_timestamps: bool, include_speakers: bool) -> String {
    let mut output = String::new();
    output.push_str(&format!("# {}\n\n", transcript.title));
    if let Some(lang) = &transcript.language {
        output.push_str(&format!("Language: {}\n", lang));
    }
    if let Some(dur) = transcript.duration_ms {
        let mins = dur / 60000;
        let secs = (dur % 60000) / 1000;
        output.push_str(&format!("Duration: {}:{:02}\n", mins, secs));
    }
    output.push('\n');

    for seg in segments {
        if include_timestamps {
            output.push_str(&format!("[{}] ", format_timestamp_txt(seg.start_ms)));
        }
        if include_speakers {
            if let Some(ref speaker) = seg.speaker_id {
                output.push_str(&format!("{}: ", speaker));
            }
        }
        output.push_str(&seg.text);
        output.push('\n');
    }
    output
}

fn format_timestamp_txt(ms: i64) -> String {
    let total_secs = ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_transcript() -> TranscriptRow {
        TranscriptRow {
            id: "t1".into(), title: "Test".into(), created_at: 0, updated_at: 0,
            duration_ms: Some(120000), language: Some("en".into()), model_id: None,
            source_type: None, source_url: None, audio_path: None, folder_id: None,
            is_starred: false, is_deleted: false, deleted_at: None,
            speaker_count: 0, word_count: 0, metadata: "{}".into(),
        }
    }

    fn sample_segments() -> Vec<SegmentRow> {
        vec![
            SegmentRow { id: "s1".into(), transcript_id: "t1".into(), index_num: 0, start_ms: 0, end_ms: 5000, text: "Hello world".into(), speaker_id: None, confidence: Some(0.9), is_deleted: false },
            SegmentRow { id: "s2".into(), transcript_id: "t1".into(), index_num: 1, start_ms: 5000, end_ms: 10000, text: "How are you".into(), speaker_id: None, confidence: Some(0.8), is_deleted: false },
        ]
    }

    #[test]
    fn test_render_txt_with_timestamps() {
        let output = render(&sample_transcript(), &sample_segments(), true, false);
        assert!(output.contains("[00:00]"));
        assert!(output.contains("Hello world"));
    }

    #[test]
    fn test_render_txt_without_timestamps() {
        let output = render(&sample_transcript(), &sample_segments(), false, false);
        assert!(!output.contains("[00:00]"));
        assert!(output.contains("Hello world"));
    }
}
