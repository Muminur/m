use crate::database::segments::SegmentRow;

pub fn render(segments: &[SegmentRow]) -> String {
    let mut output = String::from("WEBVTT\n\n");
    for seg in segments {
        output.push_str(&format!("{} --> {}\n", format_vtt_time(seg.start_ms), format_vtt_time(seg.end_ms)));
        if let Some(ref speaker) = seg.speaker_id {
            output.push_str(&format!("<v {}>{}</v>\n", speaker, seg.text));
        } else {
            output.push_str(&seg.text);
            output.push('\n');
        }
        output.push('\n');
    }
    output
}

fn format_vtt_time(ms: i64) -> String {
    let hours = ms / 3_600_000;
    let mins = (ms % 3_600_000) / 60_000;
    let secs = (ms % 60_000) / 1000;
    let millis = ms % 1000;
    format!("{:02}:{:02}:{:02}.{:03}", hours, mins, secs, millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vtt_starts_with_header() {
        let output = render(&[]);
        assert!(output.starts_with("WEBVTT"));
    }

    #[test]
    fn test_vtt_time_format() {
        assert_eq!(format_vtt_time(61500), "00:01:01.500");
    }

    #[test]
    fn test_render_vtt() {
        let segs = vec![
            SegmentRow { id: "s1".into(), transcript_id: "t1".into(), index_num: 0, start_ms: 0, end_ms: 2500, text: "Hello".into(), speaker_id: None, confidence: None, is_deleted: false },
        ];
        let output = render(&segs);
        assert!(output.contains("00:00:00.000 --> 00:00:02.500\nHello"));
    }
}
