//! Resume exporter for Veridicus — multi-format output (PDF/DOCX/ODT).
//!
//! Exports ATS-optimized resume markdown to PDF via Typst sidecar,
//! DOCX via manual XML generation, and ODT via manual XML generation.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tauri::AppHandle;

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
    app: &AppHandle,
) -> Result<ExportResult, String> {
    fs::create_dir_all(output_dir).map_err(|e| format!("Failed to create output dir: {}", e))?;

    let base_name = format!("career_resume_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"));

    match format {
        ExportFormat::Pdf => {
            let output_dir = PathBuf::from(output_dir)
                .canonicalize()
                .map_err(|e| format!("Failed to resolve output directory: {}", e))?;
            let typ_path = output_dir.join(format!("{}.typ", base_name));
            fs::write(
                &typ_path,
                crate::typst_engine::with_document_font_defaults(markdown),
            ).map_err(|e| format!("Failed to write .typ: {}", e))?;

            let (typst_path, mut command) =
                crate::typst_engine::sidecar_command(app, "typst", &output_dir)?;
            let output = command
                .arg("compile")
                .arg(&typ_path)
                .arg(output_dir.join(format!("{}.pdf", base_name)))
                .output()
                .map_err(|error| format!(
                    "Failed to execute bundled Typst binary at {} from {}: {}",
                    typst_path.display(),
                    output_dir.display(),
                    error
                ))?;

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

            let pdf_path = output_dir.join(format!("{}.pdf", base_name));
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
    use zip::write::SimpleFileOptions;

    let doc_content = generate_docx_xml(markdown);

    let file = fs::File::create(output_path).map_err(|e| format!("Failed to create DOCX: {}", e))?;
    let mut zip = ZipWriter::new(file);

    let options = SimpleFileOptions::default()
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
<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>"#
        .as_bytes(),
    )
    .map_err(|e| format!("Failed to write content types: {}", e))?;

    zip.start_file("_rels/.rels", options)
        .map_err(|e| format!("Failed to write rels: {}", e))?;
    zip.write_all(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#
        .as_bytes(),
    )
    .map_err(|e| format!("Failed to write package relationships: {}", e))?;

    zip.start_file("word/_rels/document.xml.rels", options)
        .map_err(|e| format!("Failed to write document relationships: {}", e))?;
    zip.write_all(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#
        .as_bytes(),
    )
    .map_err(|e| format!("Failed to write document relationships: {}", e))?;

    zip.start_file("word/document.xml", options)
        .map_err(|e| format!("Failed to write document.xml: {}", e))?;
    zip.write_all(doc_content.as_bytes())
        .map_err(|e| format!("Failed to write document.xml: {}", e))?;

    zip.start_file("word/styles.xml", options)
        .map_err(|e| format!("Failed to write styles.xml: {}", e))?;
    zip.write_all(generate_docx_styles_xml().as_bytes())
        .map_err(|e| format!("Failed to write styles.xml: {}", e))?;

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
                r#"<w:p><w:pPr><w:pStyle w:val="ListParagraph"/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>"#,
                escape_xml(&format!("• {}", text))
            ));
        } else {
            xml.push_str(&format!(
                r#"<w:p><w:r><w:t>{}</w:t></w:r></w:p>"#,
                escape_xml(trimmed)
            ));
        }
    }

    xml.push_str(
        r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#,
    );
    xml
}

fn generate_docx_styles_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/></w:pPr><w:rPr><w:b/><w:sz w:val="26"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:pPr><w:ind w:left="360"/></w:pPr></w:style>
</w:styles>"#
        .to_string()
}

/// Escape XML special characters.
fn escape_xml(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\t' | '\n' | '\r' => escaped.push(character),
            character
                if ('\u{20}'..='\u{D7FF}').contains(&character)
                    || ('\u{E000}'..='\u{FFFD}').contains(&character)
                    || ('\u{10000}'..='\u{10FFFF}').contains(&character) =>
            {
                escaped.push(character);
            }
            _ => {}
        }
    }
    escaped
}

/// Generate ODT from Markdown using manual XML (ZIP-based ODT).
fn generate_odt(markdown: &str, output_path: &PathBuf) -> Result<(), String> {
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    let content_xml = generate_odt_content_xml(markdown);
    let styles_xml = generate_odt_styles_xml();
    let meta_xml = generate_odt_meta_xml();
    let settings_xml = generate_odt_settings_xml();
    let manifest_xml = generate_odt_manifest_xml();

    let file = fs::File::create(output_path).map_err(|e| format!("Failed to create ODT: {}", e))?;
    let mut zip = ZipWriter::new(file);

    // ODF 1.2 §3.2: `mimetype` is the first entry and is stored, not deflated,
    // so unzip and file(1) can read the media type without inflating the archive.
    let stored = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o644);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    zip.start_file("mimetype", stored)
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
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" office:version="1.2">
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

    xml.push_str("</office:text></office:body></office:document-content>");
    xml
}

/// Generate ODT styles.xml.
fn generate_odt_styles_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" office:version="1.2">
<office:styles>
<style:default-style style:family="paragraph">
<style:text-properties fo:font-family="Calibri" fo:font-size="11pt"/>
</style:default-style>
<style:style style:name="Standard" style:family="paragraph">
<style:paragraph-properties fo:margin-bottom="0.08in"/>
</style:style>
<style:style style:name="Heading1" style:family="paragraph">
<style:paragraph-properties fo:margin-top="0.16in" fo:margin-bottom="0.08in" fo:keep-with-next="always"/>
<style:text-properties fo:font-size="16pt" fo:font-weight="bold"/>
</style:style>
<style:style style:name="Heading2" style:family="paragraph">
<style:paragraph-properties fo:margin-top="0.12in" fo:margin-bottom="0.06in" fo:keep-with-next="always"/>
<style:text-properties fo:font-size="13pt" fo:font-weight="bold"/>
</style:style>
<style:style style:name="List" style:family="paragraph">
<style:paragraph-properties fo:margin-left="0.25in" fo:text-indent="-0.25in"/>
</style:style>
</office:styles>
</office:document-styles>"#
    .to_string()
}

/// Generate ODT meta.xml.
fn generate_odt_meta_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" office:version="1.2">
<office:meta>
<meta:generator>Veridicus</meta:generator>
<dc:title>Veridicus Resume</dc:title>
<dc:date>{}</dc:date>
</office:meta>
</office:document-meta>"#,
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
    )
}

/// Generate ODT settings.xml.
fn generate_odt_settings_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-settings xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" office:version="1.2">
<office:settings/>
</office:document-settings>"#
    .to_string()
}

/// Generate ODT manifest.xml.
fn generate_odt_manifest_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2">
<manifest:file-entry manifest:media-type="application/vnd.oasis.opendocument.text" manifest:full-path="/" manifest:version="1.2"/>
<manifest:file-entry manifest:media-type="text/xml" manifest:full-path="content.xml"/>
<manifest:file-entry manifest:media-type="text/xml" manifest:full-path="styles.xml"/>
<manifest:file-entry manifest:media-type="text/xml" manifest:full-path="meta.xml"/>
<manifest:file-entry manifest:media-type="text/xml" manifest:full-path="settings.xml"/>
</manifest:manifest>"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn generated_docx_has_valid_package_relationships_and_document_parts() {
        let output_path = std::env::temp_dir().join(format!(
            "veridicus-docx-test-{}.docx",
            uuid::Uuid::new_v4()
        ));
        generate_docx(
            "# Resume\n\n## Experience\n\n- Built reliable tools",
            &output_path,
        )
        .expect("DOCX generation should succeed");

        let file = fs::File::open(&output_path).expect("generated DOCX should exist");
        let mut archive = zip::ZipArchive::new(file).expect("DOCX should be a ZIP archive");
        let mut read_entry = |name: &str| {
            let mut content = String::new();
            archive
                .by_name(name)
                .unwrap_or_else(|_| panic!("DOCX missing {name}"))
                .read_to_string(&mut content)
                .expect("DOCX XML should be readable");
            content
        };

        let package_relationships = read_entry("_rels/.rels");
        assert!(package_relationships.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml""#
        ));
        let document_relationships = read_entry("word/_rels/document.xml.rels");
        assert!(document_relationships.contains(r#"Target="styles.xml""#));
        let document = read_entry("word/document.xml");
        assert!(document.contains("<w:sectPr>"));
        assert!(document.contains("• Built reliable tools"));
        assert!(!document.contains("<w:numPr>"));
        assert!(read_entry("word/styles.xml").contains(r#"w:styleId="Heading1""#));
        assert!(read_entry("[Content_Types].xml").contains(
            r#"PartName="/word/styles.xml""#
        ));

        drop(archive);
        fs::remove_file(output_path).expect("temporary DOCX should be removed");
    }

    #[test]
    fn xml_escaping_removes_illegal_xml_control_characters() {
        assert_eq!(escape_xml("A\u{1} & <B>"), "A &amp; &lt;B&gt;");
    }

    /// The exporter and the importer share the `zip` crate, so this is the only
    /// test that would catch a writer/reader split after a crate upgrade.
    #[test]
    fn docx_export_reads_back_through_the_parser() {
        let output_path = std::env::temp_dir().join(format!(
            "veridicus-docx-roundtrip-{}.docx",
            uuid::Uuid::new_v4()
        ));
        generate_docx(
            "# Resume\n\n## Experience\n\n- Built reliable tools",
            &output_path,
        )
        .expect("DOCX generation should succeed");

        let text = crate::resume_parser::parse_docx_text(
            output_path
                .to_str()
                .expect("the temporary path should be UTF-8"),
        )
        .expect("the DOCX just written should parse");

        assert!(text.contains("Resume"), "missing heading: {text}");
        assert!(text.contains("Experience"), "missing section: {text}");
        assert!(
            text.contains("Built reliable tools"),
            "missing bullet: {text}"
        );

        fs::remove_file(output_path).expect("temporary DOCX should be removed");
    }

    #[test]
    fn odt_export_reads_back_through_the_parser() {
        let output_path = std::env::temp_dir().join(format!(
            "veridicus-odt-roundtrip-{}.odt",
            uuid::Uuid::new_v4()
        ));
        generate_odt(
            "# Resume\n\n## Experience\n\n- Built reliable tools",
            &output_path,
        )
        .expect("ODT generation should succeed");

        let text = crate::resume_parser::parse_odt_text(
            output_path
                .to_str()
                .expect("the temporary path should be UTF-8"),
        )
        .expect("the ODT just written should parse");

        assert!(text.contains("Resume"), "missing heading: {text}");
        assert!(text.contains("Experience"), "missing section: {text}");
        assert!(
            text.contains("Built reliable tools"),
            "missing bullet: {text}"
        );

        fs::remove_file(output_path).expect("temporary ODT should be removed");
    }

    /// ODF 1.2 says `mimetype` is the first entry and is stored, each part has
    /// its own document root, and the manifest lists the parts. A word
    /// processor opens an archive that gets this wrong as an empty document,
    /// which is what happened when `content.xml` was rooted at
    /// `<office:document>` and every part was deflated.
    #[test]
    fn odt_package_matches_the_opendocument_container_rules() {
        let output_path = std::env::temp_dir().join(format!(
            "veridicus-odt-container-{}.odt",
            uuid::Uuid::new_v4()
        ));
        generate_odt(
            "# Resume\n\n## Experience\n\n- Built reliable tools",
            &output_path,
        )
        .expect("ODT generation should succeed");

        let file = fs::File::open(&output_path).expect("generated ODT should exist");
        let mut archive = zip::ZipArchive::new(file).expect("ODT should be a ZIP archive");

        {
            let first = archive.by_index(0).expect("ODT should have entries");
            assert_eq!(first.name().expect("entry name").as_ref(), "mimetype");
            assert_eq!(first.compression(), zip::CompressionMethod::Stored);
        }

        let mut read_entry = |name: &str| {
            let mut content = String::new();
            archive
                .by_name(name)
                .unwrap_or_else(|_| panic!("ODT missing {name}"))
                .read_to_string(&mut content)
                .expect("ODT XML should be readable");
            content
        };

        assert_eq!(read_entry("mimetype"), "application/vnd.oasis.opendocument.text");

        let content = read_entry("content.xml");
        assert!(
            content.contains("<office:document-content"),
            "content.xml needs the document-content root: {content}"
        );
        assert!(content.contains("<office:body>"), "{content}");
        assert!(content.contains("<office:text>"), "{content}");

        assert!(read_entry("styles.xml").contains("<office:document-styles"));
        assert!(read_entry("meta.xml").contains("<office:document-meta"));
        assert!(read_entry("settings.xml").contains("<office:document-settings"));

        let manifest = read_entry("META-INF/manifest.xml");
        for part in ["content.xml", "styles.xml", "meta.xml", "settings.xml"] {
            assert!(
                manifest.contains(&format!(r#"manifest:full-path="{part}""#)),
                "manifest is missing {part}: {manifest}"
            );
        }

        drop(archive);
        fs::remove_file(output_path).expect("temporary ODT should be removed");
    }
}
