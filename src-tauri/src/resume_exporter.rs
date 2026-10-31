//! Resume exporter for CareerOS — multi-format output (PDF/DOCX/ODT).
//!
//! Exports ATS-optimized resume markdown to PDF via Typst sidecar,
//! DOCX via manual XML generation, and ODT via manual XML generation.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

/// Export format options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExportFormat {
    #[serde(rename = "pdf")]
    Pdf,
    #[serde(rename = "docx")]
    Docx,
    #[serde(rename = "odt")]
    Odt,
}

/// Result of resume export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub success: bool,
    pub output_path: Option<String>,
    pub format: Option<ExportFormat>,
    pub error_message: Option<String>,
}

/// Export an ATS-optimized resume to the specified format.
pub fn export_ats_resume(
    markdown: &str,
    format: ExportFormat,
    output_dir: &str,
) -> Result<ExportResult, String> {
    fs::create_dir_all(output_dir).map_err(|e| format!("Failed to create output dir: {}", e))?;

    let base_name = format!("career_resume_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"));

    match format {
        ExportFormat::Pdf => {
            let typ_path = PathBuf::from(output_dir).join(format!("{}.typ", base_name));
            fs::write(&typ_path, markdown).map_err(|e| format!("Failed to write .typ: {}", e))?;

            let output = Command::new("sidecars/typst")
                .arg("compile")
                .arg(&typ_path)
                .arg(PathBuf::from(output_dir).join(format!("{}.pdf", base_name)))
                .output()
                .map_err(|e| format!("Failed to execute Typst: {}", e))?;

            if !output.status.success() {
                return Ok(ExportResult {
                    success: false,
                    output_path: None,
                    format: Some(ExportFormat::Pdf),
                    error_message: Some(format!(
                        "Typst compile failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )),
                });
            }

            let pdf_path = PathBuf::from(output_dir).join(format!("{}.pdf", base_name));
            Ok(ExportResult {
                success: true,
                output_path: Some(pdf_path.to_string_lossy().into_owned()),
                format: Some(ExportFormat::Pdf),
                error_message: None,
            })
        }
        ExportFormat::Docx => {
            let docx_path = PathBuf::from(output_dir).join(format!("{}.docx", base_name));
            generate_docx(markdown, &docx_path)?;
            Ok(ExportResult {
                success: true,
                output_path: Some(docx_path.to_string_lossy().into_owned()),
                format: Some(ExportFormat::Docx),
                error_message: None,
            })
        }
        ExportFormat::Odt => {
            let odt_path = PathBuf::from(output_dir).join(format!("{}.odt", base_name));
            generate_odt(markdown, &odt_path)?;
            Ok(ExportResult {
                success: true,
                output_path: Some(odt_path.to_string_lossy().into_owned()),
                format: Some(ExportFormat::Odt),
                error_message: None,
            })
        }
    }
}

/// Generate DOCX from Markdown using manual XML (ZIP-based DOCX).
fn generate_docx(markdown: &str, output_path: &PathBuf) -> Result<(), String> {
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::FileOptions;

    let doc_content = generate_docx_xml(markdown);

    let file = fs::File::create(output_path).map_err(|e| format!("Failed to create DOCX: {}", e))?;
    let mut zip = ZipWriter::new(file);

    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    // [Content_Types].xml
    zip.start_file("[Content_Types].xml", options)
        .map_err(|e| format!("Failed to write content types: {}", e))?;
    zip.write_all(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#
        .as_bytes(),
    )
    .map_err(|e| format!("Failed to write content types: {}", e))?;

    // word/_rels/document.xml.rels
    zip.start_file("word/_rels/document.xml.rels", options)
        .map_err(|e| format!("Failed to write rels: {}", e))?;
    zip.write_all(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="document.xml"/>
</Relationships>"#
        .as_bytes(),
    )
    .map_err(|e| format!("Failed to write rels: {}", e))?;

    // word/document.xml
    zip.start_file("word/document.xml", options)
        .map_err(|e| format!("Failed to write document.xml: {}", e))?;
    zip.write_all(doc_content.as_bytes())
        .map_err(|e| format!("Failed to write document.xml: {}", e))?;

    zip.finish().map_err(|e| format!("Failed to finish ZIP: {}", e))?;
    Ok(())
}

/// Generate DOCX XML from Markdown (simple conversion).
fn generate_docx_xml(markdown: &str) -> String {
    let mut xml = String::new();

    xml.push_str(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>"#,
    );

    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("# ") {
            let text = &trimmed[2..];
            xml.push_str(&format!(
                r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>"#,
                escape_xml(text)
            ));
        } else if trimmed.starts_with("## ") {
            let text = &trimmed[3..];
            xml.push_str(&format!(
                r#"<w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>"#,
                escape_xml(text)
            ));
        } else if trimmed.starts_with("- ") {
            let text = &trimmed[2..];
            xml.push_str(&format!(
                r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>"#,
                escape_xml(text)
            ));
        } else {
            xml.push_str(&format!(
                r#"<w:p><w:r><w:t>{}</w:t></w:r></w:p>"#,
                escape_xml(trimmed)
            ));
        }
    }

    xml.push_str("</w:body></w:document>");
    xml
}

/// Escape XML special characters.
fn escape_xml(text: &str) -> String {
    text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&apos;")
}

/// Generate ODT from Markdown using manual XML (ZIP-based ODT).
fn generate_odt(markdown: &str, output_path: &PathBuf) -> Result<(), String> {
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::FileOptions;

    let content_xml = generate_odt_content_xml(markdown);
    let styles_xml = generate_odt_styles_xml();
    let meta_xml = generate_odt_meta_xml();
    let settings_xml = generate_odt_settings_xml();
    let manifest_xml = generate_odt_manifest_xml();

    let file = fs::File::create(output_path).map_err(|e| format!("Failed to create ODT: {}", e))?;
    let mut zip = ZipWriter::new(file);

    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    zip.start_file("mimetype", options)
        .map_err(|e| format!("Failed to write mimetype: {}", e))?;
    zip.write_all(b"application/vnd.oasis.opendocument.text")
        .map_err(|e| format!("Failed to write mimetype: {}", e))?;

    zip.start_file("content.xml", options)
        .map_err(|e| format!("Failed to write content.xml: {}", e))?;
    zip.write_all(content_xml.as_bytes())
        .map_err(|e| format!("Failed to write content.xml: {}", e))?;

    zip.start_file("styles.xml", options)
        .map_err(|e| format!("Failed to write styles.xml: {}", e))?;
    zip.write_all(styles_xml.as_bytes())
        .map_err(|e| format!("Failed to write styles.xml: {}", e))?;

    zip.start_file("meta.xml", options)
        .map_err(|e| format!("Failed to write meta.xml: {}", e))?;
    zip.write_all(meta_xml.as_bytes())
        .map_err(|e| format!("Failed to write meta.xml: {}", e))?;

    zip.start_file("settings.xml", options)
        .map_err(|e| format!("Failed to write settings.xml: {}", e))?;
    zip.write_all(settings_xml.as_bytes())
        .map_err(|e| format!("Failed to write settings.xml: {}", e))?;

    zip.start_file("META-INF/manifest.xml", options)
        .map_err(|e| format!("Failed to write manifest.xml: {}", e))?;
    zip.write_all(manifest_xml.as_bytes())
        .map_err(|e| format!("Failed to write manifest.xml: {}", e))?;

    zip.finish().map_err(|e| format!("Failed to finish ZIP: {}", e))?;
    Ok(())
}

/// Generate ODT content.xml from Markdown.
fn generate_odt_content_xml(markdown: &str) -> String {
    let mut xml = String::new();

    xml.push_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
office:version="1.2">
<office:body>
<office:text>"#,
    );

    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("# ") {
            let text = &trimmed[2..];
            xml.push_str(&format!(
                r#"<text:p text:style-name="Heading1">{}</text:p>"#,
                escape_xml(text)
            ));
        } else if trimmed.starts_with("## ") {
            let text = &trimmed[3..];
            xml.push_str(&format!(
                r#"<text:p text:style-name="Heading2">{}</text:p>"#,
                escape_xml(text)
            ));
        } else if trimmed.starts_with("- ") {
            let text = &trimmed[2..];
            xml.push_str(&format!(
                r#"<text:p text:style-name="List">{}</text:p>"#,
                escape_xml(text)
            ));
        } else {
            xml.push_str(&format!(
                r#"<text:p text:style-name="Standard">{}</text:p>"#,
                escape_xml(trimmed)
            ));
        }
    }

    xml.push_str("</office:text></office:body></office:document>");
    xml
}

/// Generate ODT styles.xml.
fn generate_odt_styles_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
office:version="1.2">
<office:automatic-styles>
<style:style style:name="Standard" style:family="paragraph"/>
<style:style style:name="Heading1" style:family="paragraph">
<text:properties/>
</style:style>
<style:style style:name="Heading2" style:family="paragraph">
<text:properties/>
</style:style>
<style:style style:name="List" style:family="paragraph">
<text:properties/>
</style:style>
</office:automatic-styles>
</office:document-styles>"#
    .to_string()
}

/// Generate ODT meta.xml.
fn generate_odt_meta_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
office:version="1.2">
<office:meta>
<dc:title xmlns:dc="http://purl.org/dc/elements/1.1/">CareerOS Resume</dc:title>
<dc:date xmlns:dc="http://purl.org/dc/elements/1.1/"></dc:date>
</office:meta>
</office:document-meta>"#
    .to_string()
}

/// Generate ODT settings.xml.
fn generate_odt_settings_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-settings xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
office:version="1.2">
<office:settings/>
</office:document-settings>"#
    .to_string()
}

/// Generate ODT manifest.xml.
fn generate_odt_manifest_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0">
<manifest:file-entry manifest:media-type="application/vnd.oasis.opendocument.text" manifest:full-path="/"/>
<manifest:file-entry manifest:media-type="" manifest:full-path="Pictures/"/>
</manifest:manifest>"#
    .to_string()
}
