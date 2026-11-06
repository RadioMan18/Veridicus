//! Multi-format resume text extraction for CareerOS.
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

/// Extract text from XML content inside a DOCX/ODT zip archive.
/// Uses regex to extract text from w:t / text:p tags.
fn extract_text_from_xml(xml: &str) -> String {
    let mut text = String::new();

    // Extract all w:t (Word text) or text:p (ODT paragraph) content
    let re = regex::Regex::new(r"<(?:w:t|text:p)[^>]*>(.*?)</(?:w:t|text:p)>").unwrap();
    for cap in re.captures_iter(xml) {
        let content = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        // Decode basic XML entities
        let decoded = content
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&apos;", "'");
        text.push_str(&decoded);
        text.push(' ');
    }

    text.trim().to_string()
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
}
