use crate::database::segments::SegmentRow;
use crate::error::{AppError, ExportErrorCode};
use handlebars::Handlebars;
use serde::Serialize;
use std::io::Write;

/// Static OOXML template files baked into the binary at compile time.
const CONTENT_TYPES_XML: &str = include_str!("../../../templates/docx/[Content_Types].xml");
const RELS_XML: &str = include_str!("../../../templates/docx/_rels/.rels");
const STYLES_XML: &str = include_str!("../../../templates/docx/styles.xml");
const DOCUMENT_XML_HBS: &str = include_str!("../../../templates/docx/document.xml.hbs");
const DOCUMENT_RELS_XML: &str =
    include_str!("../../../templates/docx/word/_rels/document.xml.rels");

/// A segment exposed to the DOCX Handlebars template.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DocxSegment {
    index: i64,
    start_ms: i64,
    end_ms: i64,
    start_time: String,
    end_time: String,
    speaker: String,
    text: String,
}

/// Full context for the DOCX Handlebars template.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DocxContext {
    title: String,
    duration: String,
    duration_ms: i64,
    language: String,
    segment_count: usize,
    segments: Vec<DocxSegment>,
}

/// Export transcript as a DOCX file (OOXML zip archive).
///
/// Uses `handlebars` to render `document.xml` from a template, then packages
/// static OOXML parts (`styles.xml`, `[Content_Types].xml`, `_rels/.rels`) into
/// a zip archive using the `zip` crate.
///
/// Returns raw bytes of the `.docx` file.
pub fn export_docx(
    title: &str,
    segments: &[SegmentRow],
    duration_ms: i64,
    language: &str,
) -> Result<Vec<u8>, AppError> {
    // Build template context
    let docx_segments: Vec<DocxSegment> = segments
        .iter()
        .map(|seg| DocxSegment {
            index: seg.index_num,
            start_ms: seg.start_ms,
            end_ms: seg.end_ms,
            start_time: format_timestamp(seg.start_ms),
            end_time: format_timestamp(seg.end_ms),
            speaker: seg.speaker_id.clone().unwrap_or_else(|| "Speaker".into()),
            text: xml_escape(&seg.text),
        })
        .collect();

    let context = DocxContext {
        title: xml_escape(title),
        duration: format_duration(duration_ms),
        duration_ms,
        language: xml_escape(language),
        segment_count: docx_segments.len(),
        segments: docx_segments,
    };

    // Render document.xml from Handlebars template
    let mut hbs = Handlebars::new();
    hbs.register_template_string("document", DOCUMENT_XML_HBS)
        .map_err(|e| AppError::ExportError {
            code: ExportErrorCode::TemplateError,
            message: format!("DOCX template error: {}", e),
        })?;

    let document_xml = hbs.render("document", &context).map_err(|e| AppError::ExportError {
        code: ExportErrorCode::TemplateError,
        message: format!("DOCX render error: {}", e),
    })?;

    // Build the zip archive
    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let mut zip = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // [Content_Types].xml (must be at root)
    zip.start_file("[Content_Types].xml", options)
        .map_err(zip_err)?;
    zip.write_all(CONTENT_TYPES_XML.as_bytes())
        .map_err(zip_err)?;

    // _rels/.rels
    zip.start_file("_rels/.rels", options).map_err(zip_err)?;
    zip.write_all(RELS_XML.as_bytes()).map_err(zip_err)?;

    // word/document.xml (rendered)
    zip.start_file("word/document.xml", options)
        .map_err(zip_err)?;
    zip.write_all(document_xml.as_bytes()).map_err(zip_err)?;

    // word/styles.xml
    zip.start_file("word/styles.xml", options)
        .map_err(zip_err)?;
    zip.write_all(STYLES_XML.as_bytes()).map_err(zip_err)?;

    // word/_rels/document.xml.rels
    zip.start_file("word/_rels/document.xml.rels", options)
        .map_err(zip_err)?;
    zip.write_all(DOCUMENT_RELS_XML.as_bytes())
        .map_err(zip_err)?;

    let cursor = zip.finish().map_err(zip_err)?;
    Ok(cursor.into_inner())
}

fn zip_err(e: impl std::fmt::Display) -> AppError {
    AppError::ExportError {
        code: ExportErrorCode::IoError,
        message: format!("DOCX zip error: {}", e),
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
                end_ms: 10000,
                text: "Test <special> & \"chars\"".into(),
                speaker_id: None,
                confidence: None,
                is_deleted: false,
            },
        ]
    }

    #[test]
    fn test_export_docx_produces_zip() {
        let result = export_docx("Test Transcript", &sample_segments(), 60000, "en");
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
        // ZIP magic bytes: PK\x03\x04
        assert_eq!(&bytes[0..2], b"PK");
    }

    #[test]
    fn test_export_docx_contains_expected_parts() {
        let bytes = export_docx("Test", &sample_segments(), 60000, "en").unwrap();
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains(&"[Content_Types].xml".to_string()));
        assert!(names.contains(&"_rels/.rels".to_string()));
        assert!(names.contains(&"word/document.xml".to_string()));
        assert!(names.contains(&"word/styles.xml".to_string()));
        assert!(names.contains(&"word/_rels/document.xml.rels".to_string()));
    }

    #[test]
    fn test_export_docx_document_contains_content() {
        let bytes = export_docx("My Title", &sample_segments(), 60000, "en").unwrap();
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut doc = archive.by_name("word/document.xml").unwrap();
        let mut content = String::new();
        std::io::Read::read_to_string(&mut doc, &mut content).unwrap();
        assert!(content.contains("My Title"));
        assert!(content.contains("Hello world"));
        assert!(content.contains("Alice"));
    }

    #[test]
    fn test_export_docx_xml_escaping() {
        let bytes = export_docx("Test", &sample_segments(), 60000, "en").unwrap();
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut doc = archive.by_name("word/document.xml").unwrap();
        let mut content = String::new();
        std::io::Read::read_to_string(&mut doc, &mut content).unwrap();
        // XML special chars must be escaped
        assert!(content.contains("&lt;special&gt;"));
        assert!(content.contains("&amp;"));
    }

    #[test]
    fn test_export_docx_empty_segments() {
        let result = export_docx("Empty", &[], 0, "en");
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert_eq!(&bytes[0..2], b"PK");
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("a&b<c>d\"e'f"), "a&amp;b&lt;c&gt;d&quot;e&apos;f");
    }
}
