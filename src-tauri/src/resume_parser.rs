//! Multi-format resume text extraction for Veridicus.
//!
//! Supports PDF (via pdftotext sidecar), DOCX (via zip + regex), and ODT
//! (via zip + regex). Extracts plain text suitable for claim parsing and
//! ATS-optimized resume generation.

use std::path::Path;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

/// Represents the format of a resume file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ResumeFormat {
    Pdf,
    Docx,
    Odt,
    Text,
    Unknown,
}

/// Detect resume format from file extension.
pub fn detect_format(path: &str) -> ResumeFormat {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "pdf" => ResumeFormat::Pdf,
        "docx" => ResumeFormat::Docx,
        "odt" => ResumeFormat::Odt,
        "txt" | "md" | "markdown" => ResumeFormat::Text,
        _ => ResumeFormat::Unknown,
    }
}

/// Extract text from a PDF file using the bundled pdftotext sidecar.
pub fn parse_pdf_text(path: &str, app: &AppHandle) -> Result<String, String> {
    let pdf_path = Path::new(path)
        .canonicalize()
        .map_err(|error| format!("Failed to resolve PDF path: {}", error))?;
    let working_directory = pdf_path
        .parent()
        .ok_or("PDF file has no parent directory")?;
    let (pdftotext_path, mut command) =
        crate::typst_engine::sidecar_command(app, "pdftotext", working_directory)?;
    let output = command
        .arg("-layout")
        .arg(&pdf_path)
        .arg("-")
        .output()
        .map_err(|error| format!(
            "Failed to execute bundled pdftotext at {} from {}: {}",
            pdftotext_path.display(),
            working_directory.display(),
            error
        ))?;

    if !output.status.success() {
        return Err(format!(
            "pdftotext failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Extract plain text from XML content inside a DOCX/ODT zip archive.
///
/// Paragraph ends become newlines and every other tag is dropped. Capturing
/// `<w:t>` runs with a loose regex is not enough: a bare `w:t` prefix also
/// matches `<w:tblPr>`, `<w:tblGrid>`, `<w:tc>` and friends, which dragged raw
/// WordprocessingML into imported resumes and from there into the claims ledger.
pub fn extract_text_from_xml(xml: &str) -> String {
    let paragraph_end = regex::Regex::new(r"</(?:w:p|text:p|text:h)>").unwrap();
    let tag = regex::Regex::new(r"<[^>]*>").unwrap();

    let mut text = String::new();
    for paragraph in paragraph_end.split(xml) {
        let stripped = tag.replace_all(paragraph, " ");
        let line = decode_entities(&stripped)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if line.is_empty() {
            continue;
        }
        text.push_str(&line);
        text.push('\n');
    }

    text.trim().to_string()
}

/// Decode the XML entities that appear in document text.
fn decode_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Extract text from a DOCX file using zip + regex.
pub fn parse_docx_text(path: &str) -> Result<String, String> {
    use std::io::Read;

    let file = std::fs::File::open(path).map_err(|e| format!("Failed to open DOCX: {}", e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("Failed to open DOCX zip: {}", e))?;

    // Find document.xml in the zip
    let mut document_xml = String::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| format!("Failed to read zip entry: {}", e))?;
        if entry.name() == "word/document.xml" {
            entry.read_to_string(&mut document_xml).map_err(|e| format!("Failed to read document.xml: {}", e))?;
            break;
        }
    }

    if document_xml.is_empty() {
        return Err("document.xml not found in DOCX".to_string());
    }

    Ok(extract_text_from_xml(&document_xml))
}

/// Extract text from an ODT file using zip + regex.
pub fn parse_odt_text(path: &str) -> Result<String, String> {
    use std::io::Read;

    let file = std::fs::File::open(path).map_err(|e| format!("Failed to open ODT: {}", e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("Failed to open ODT zip: {}", e))?;

    // Find content.xml in the zip
    let mut content_xml = String::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| format!("Failed to read zip entry: {}", e))?;
        if entry.name() == "content.xml" {
            entry.read_to_string(&mut content_xml).map_err(|e| format!("Failed to read content.xml: {}", e))?;
            break;
        }
    }

    if content_xml.is_empty() {
        return Err("content.xml not found in ODT".to_string());
    }

    Ok(extract_text_from_xml(&content_xml))
}

/// Extract text from a resume file, auto-detecting format.
pub fn extract_resume_text(
    path: &str,
    app: &AppHandle,
) -> Result<(ResumeFormat, String), String> {
    let format = detect_format(path);

    match format {
        ResumeFormat::Pdf => {
            let text = parse_pdf_text(path, app)?;
            Ok((format, text))
        }
        ResumeFormat::Docx => {
            let text = parse_docx_text(path)?;
            Ok((format, text))
        }
        ResumeFormat::Odt => {
            let text = parse_odt_text(path)?;
            Ok((format, text))
        }
        ResumeFormat::Text => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read resume text file: {}", e))?;
            Ok((format, text))
        }
        ResumeFormat::Unknown => Err(format!(
            "Unsupported file format: {}. Supported: .pdf, .docx, .odt, .md, .txt",
            path
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_format() {
        assert_eq!(detect_format("resume.pdf"), ResumeFormat::Pdf);
        assert_eq!(detect_format("resume.docx"), ResumeFormat::Docx);
        assert_eq!(detect_format("resume.odt"), ResumeFormat::Odt);
        assert_eq!(detect_format("resume.txt"), ResumeFormat::Text);
        assert_eq!(detect_format("resume.md"), ResumeFormat::Text);
        assert_eq!(detect_format("resume.PDF"), ResumeFormat::Pdf);
    }

    #[test]
    fn test_extract_text_from_xml() {
        let xml = r#"<w:document><w:body><w:p><w:t>Hello</w:t></w:p><w:p><w:t>World</w:t></w:p></w:body></w:document>"#;
        let text = extract_text_from_xml(xml);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn table_markup_never_reaches_the_text() {
        let xml = r#"<w:document><w:body><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>Hello</w:t></w:r></w:p><w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="10800"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>Cell text</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#;
        let text = extract_text_from_xml(xml);
        assert!(text.contains("Hello"));
        assert!(text.contains("Cell text"));
        assert!(!text.contains("w:tblPr"));
        assert!(!text.contains("w:gridCol"));
        assert!(!text.contains('<'), "markup leaked: {text}");
    }

    #[test]
    fn odt_paragraphs_keep_their_line_breaks() {
        let xml = r#"<office:body><text:p>Alpha <text:span>beta</text:span></text:p><text:p>Gamma &amp; delta</text:p></office:body>"#;
        let text = extract_text_from_xml(xml);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines, vec!["Alpha beta", "Gamma & delta"]);
    }
}
