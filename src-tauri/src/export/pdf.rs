use crate::database::segments::SegmentRow;
use crate::error::{AppError, ExportErrorCode};
use printpdf::*;
use std::io::BufWriter;

/// A4 dimensions in mm
const A4_WIDTH_MM: f32 = 210.0;
const A4_HEIGHT_MM: f32 = 297.0;

/// Margins in mm
const MARGIN_LEFT: f32 = 25.0;
const MARGIN_RIGHT: f32 = 25.0;
const MARGIN_TOP: f32 = 25.0;
const MARGIN_BOTTOM: f32 = 25.0;

/// Font sizes in pt
const HEADER_FONT_SIZE: f32 = 14.0;
const BODY_FONT_SIZE: f32 = 11.0;
const META_FONT_SIZE: f32 = 9.0;

/// Line height multiplier
const LINE_HEIGHT_FACTOR: f32 = 1.4;

/// Export transcript segments as a PDF document.
///
/// Returns the raw PDF bytes suitable for writing to a file or base64-encoding for IPC.
pub fn export_pdf(
    title: &str,
    segments: &[SegmentRow],
    duration_ms: i64,
    language: &str,
) -> Result<Vec<u8>, AppError> {
    let doc = PdfDocument::empty(title);

    let font = doc.add_builtin_font(BuiltinFont::Helvetica).map_err(|e| AppError::ExportError {
        code: ExportErrorCode::FormatError,
        message: format!("Failed to add font: {}", e),
    })?;
    let font_bold = doc.add_builtin_font(BuiltinFont::HelveticaBold).map_err(|e| AppError::ExportError {
        code: ExportErrorCode::FormatError,
        message: format!("Failed to add bold font: {}", e),
    })?;

    let usable_width = A4_WIDTH_MM - MARGIN_LEFT - MARGIN_RIGHT;
    // Approximate chars per line for Helvetica at body size (rough: 0.5 * font_size pt = width per char in mm at ~2.83 mm/pt)
    let approx_char_width_mm = BODY_FONT_SIZE * 0.22;
    let max_chars_per_line = (usable_width / approx_char_width_mm) as usize;

    let mut pages: Vec<Vec<PdfLine>> = Vec::new();
    let mut current_page_lines: Vec<PdfLine> = Vec::new();
    let page_body_height = A4_HEIGHT_MM - MARGIN_TOP - MARGIN_BOTTOM;

    // Header lines
    current_page_lines.push(PdfLine {
        text: title.to_string(),
        font_size: HEADER_FONT_SIZE,
        bold: true,
    });
    current_page_lines.push(PdfLine {
        text: String::new(),
        font_size: BODY_FONT_SIZE,
        bold: false,
    });

    // Metadata line
    let duration_str = format_duration(duration_ms);
    let meta = format!("Language: {}  |  Duration: {}", language, duration_str);
    current_page_lines.push(PdfLine {
        text: meta,
        font_size: META_FONT_SIZE,
        bold: false,
    });
    current_page_lines.push(PdfLine {
        text: String::new(),
        font_size: BODY_FONT_SIZE,
        bold: false,
    });

    let mut current_height = estimate_lines_height(&current_page_lines);

    for seg in segments {
        let speaker = seg.speaker_id.as_deref().unwrap_or("Speaker");
        let timestamp = format_timestamp(seg.start_ms);
        let prefix = format!("[{}] {}: ", timestamp, speaker);
        let full_text = format!("{}{}", prefix, seg.text.trim());

        let wrapped = word_wrap(&full_text, max_chars_per_line);
        let line_height = BODY_FONT_SIZE * LINE_HEIGHT_FACTOR * 0.3528; // pt to mm

        for wrapped_line in &wrapped {
            let needed = line_height;
            if current_height + needed > page_body_height {
                pages.push(std::mem::take(&mut current_page_lines));
                current_height = 0.0;
            }
            current_page_lines.push(PdfLine {
                text: wrapped_line.clone(),
                font_size: BODY_FONT_SIZE,
                bold: false,
            });
            current_height += needed;
        }

        // Add blank line between segments
        let blank_height = BODY_FONT_SIZE * LINE_HEIGHT_FACTOR * 0.3528;
        if current_height + blank_height <= page_body_height {
            current_page_lines.push(PdfLine {
                text: String::new(),
                font_size: BODY_FONT_SIZE,
                bold: false,
            });
            current_height += blank_height;
        }
    }

    if !current_page_lines.is_empty() {
        pages.push(current_page_lines);
    }

    // If no content at all, add at least one page
    if pages.is_empty() {
        pages.push(vec![PdfLine {
            text: title.to_string(),
            font_size: HEADER_FONT_SIZE,
            bold: true,
        }]);
    }

    // Render pages
    for (page_idx, page_lines) in pages.iter().enumerate() {
        let (page, layer_idx) = if page_idx == 0 {
            doc.add_page(
                Mm(A4_WIDTH_MM),
                Mm(A4_HEIGHT_MM),
                &format!("Page {}", page_idx + 1),
            )
        } else {
            doc.add_page(
                Mm(A4_WIDTH_MM),
                Mm(A4_HEIGHT_MM),
                &format!("Page {}", page_idx + 1),
            )
        };

        let layer = doc.get_page(page).get_layer(layer_idx);
        let mut y_pos = A4_HEIGHT_MM - MARGIN_TOP;

        for line in page_lines {
            if line.text.is_empty() {
                y_pos -= line.font_size * LINE_HEIGHT_FACTOR * 0.3528;
                continue;
            }

            let current_font = if line.bold { &font_bold } else { &font };

            layer.use_text(
                &line.text,
                line.font_size,
                Mm(MARGIN_LEFT),
                Mm(y_pos),
                current_font,
            );
            y_pos -= line.font_size * LINE_HEIGHT_FACTOR * 0.3528;
        }
    }

    let mut buf = BufWriter::new(Vec::new());
    doc.save(&mut buf).map_err(|e| AppError::ExportError {
        code: ExportErrorCode::FormatError,
        message: format!("Failed to save PDF: {}", e),
    })?;

    buf.into_inner().map_err(|e| AppError::ExportError {
        code: ExportErrorCode::IoError,
        message: format!("Failed to flush PDF buffer: {}", e),
    })
}

struct PdfLine {
    text: String,
    font_size: f32,
    bold: bool,
}

fn estimate_lines_height(lines: &[PdfLine]) -> f32 {
    lines
        .iter()
        .map(|l| l.font_size * LINE_HEIGHT_FACTOR * 0.3528)
        .sum()
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

fn word_wrap(text: &str, max_chars: usize) -> Vec<String> {
    if max_chars == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= max_chars {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current_line));
            current_line = word.to_string();
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
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
                text: "Hello world, this is a test transcript.".into(),
                speaker_id: Some("Alice".into()),
                confidence: Some(0.95),
                is_deleted: false,
            },
            SegmentRow {
                id: "s2".into(),
                transcript_id: "t1".into(),
                index_num: 1,
                start_ms: 5000,
                end_ms: 12000,
                text: "And this is the second segment of the transcript.".into(),
                speaker_id: Some("Bob".into()),
                confidence: Some(0.88),
                is_deleted: false,
            },
        ]
    }

    #[test]
    fn test_export_pdf_produces_bytes() {
        let result = export_pdf("Test Transcript", &sample_segments(), 60000, "en");
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
        // PDF magic bytes: %PDF
        assert_eq!(&bytes[0..5], b"%PDF-");
    }

    #[test]
    fn test_export_pdf_empty_segments() {
        let result = export_pdf("Empty", &[], 0, "en");
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(&bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0), "00:00:00");
        assert_eq!(format_timestamp(61500), "00:01:01");
        assert_eq!(format_timestamp(3723000), "01:02:03");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(60000), "1m 0s");
        assert_eq!(format_duration(3661000), "1h 1m 1s");
    }

    #[test]
    fn test_word_wrap() {
        let lines = word_wrap("hello world foo bar baz", 12);
        assert!(lines.len() >= 2);
        for line in &lines {
            assert!(line.len() <= 15); // allow some tolerance for single long words
        }
    }

    #[test]
    fn test_word_wrap_single_long_word() {
        let lines = word_wrap("superlongword", 5);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "superlongword");
    }
}
