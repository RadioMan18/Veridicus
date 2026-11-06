//! Typst Engine & Visual QA for CareerOS.
//!
//! Resolves bundled sidecar binaries (typst, pdftoppm, pdftotext) from the app
//! bundle, compiles .typ to .pdf, rasterizes pages to PNG, and runs layout
//! analysis for visual review gate flags.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{path::BaseDirectory, AppHandle, Manager};

const DOCUMENT_MARKER: &str = "// CareerOS generated Typst document";
const LEGACY_FONT_MARKER: &str = "// CareerOS default font stack from color_palette/aether-fonts.css";
const DOCUMENT_PREAMBLE: &str = concat!(
    "// CareerOS generated Typst document\n",
    "#set page(margin: (x: 0.8in, y: 0.7in))\n",
    "#set text(font: (\"Segoe UI\", \"Arial\", \"Liberation Sans\"), size: 10pt)\n",
    "#set par(leading: 0.72em, spacing: 0.65em)\n",
    "#set heading(numbering: none)\n",
    "#set list(spacing: 0.35em)\n",
    "#show heading.where(level: 1): set text(size: 20pt)\n",
    "#show heading.where(level: 2): set text(size: 14pt)\n",
    "#show heading.where(level: 3): set text(size: 11pt)\n\n"
);

/// Result of compiling a Typst file to PDF.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileResult {
    pub pdf_path: String,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Result of rasterizing PDF pages to PNG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RasterizeResult {
    pub png_paths: Vec<String>,
    pub page_count: u32,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Visual QA flags detected during layout analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualFlags {
    pub overflow_flags: Vec<String>,
    pub orphan_heading_flags: Vec<String>,
    pub broken_wrap_flags: Vec<String>,
    pub placeholder_flags: Vec<String>,
    pub metadata_flags: Vec<String>,
}

pub(crate) fn with_document_font_defaults(source: &str) -> String {
    if source.starts_with(DOCUMENT_MARKER) {
        source.to_string()
    } else {
        let markdown = if source.starts_with(LEGACY_FONT_MARKER) {
            source
                .lines()
                .skip(2)
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            source.to_string()
        };
        format!("{}{}", DOCUMENT_PREAMBLE, markdown_to_typst(&markdown))
    }
}

fn markdown_to_typst(markdown: &str) -> String {
    let arena = comrak::Arena::new();
    let mut options = comrak::Options::default();
    // GFM tables are common in imported resumes; without this they render as raw pipe text.
    options.extension.table = true;
    let root = comrak::parse_document(&arena, markdown, &options);
    let mut output = String::new();
    render_blocks(root, &mut output, 0);
    output
}

fn render_blocks<'a>(
    node: &'a comrak::nodes::AstNode<'a>,
    output: &mut String,
    depth: usize,
) {
    use comrak::nodes::NodeValue;

    for child in node.children() {
        match &child.data.borrow().value {
            NodeValue::Document => render_blocks(child, output, depth),
            NodeValue::Paragraph => {
                let mut paragraph = String::new();
                render_inlines(child, &mut paragraph);
                let paragraph = paragraph.trim();
                if !paragraph.is_empty() {
                    output.push_str(paragraph);
                    output.push_str("\n\n");
                }
            }
            NodeValue::Heading(heading) => {
                output.push_str(&"=".repeat(heading.level.min(4) as usize));
                output.push(' ');
                render_inlines(child, output);
                output.push_str("\n\n");
            }
            NodeValue::List(list) => {
                let marker = match list.list_type {
                    comrak::nodes::ListType::Bullet => "-",
                    comrak::nodes::ListType::Ordered => "+",
                };
                for item in child.children() {
                    if !matches!(item.data.borrow().value, NodeValue::Item(_)) {
                        continue;
                    }
                    output.push_str(&"  ".repeat(depth));
                    output.push_str(marker);
                    output.push(' ');
                    render_list_item(item, output, depth);
                }
                output.push('\n');
            }
            NodeValue::BlockQuote => render_blocks(child, output, depth),
            NodeValue::ThematicBreak => output.push_str("#line(length: 100%)\n\n"),
            NodeValue::CodeBlock(code) => {
                for line in code.literal.lines() {
                    output.push_str(&escape_typst_text(line));
                    output.push('\n');
                }
                output.push('\n');
            }
            NodeValue::HtmlBlock(html) => {
                output.push_str(&escape_typst_text(&html.literal));
                output.push_str("\n\n");
            }
            NodeValue::Table(_) => {
                let mut rows: Vec<Vec<String>> = Vec::new();
                for row in child.children() {
                    let mut cells: Vec<String> = Vec::new();
                    for cell in row.children() {
                        let mut content = String::new();
                        render_inlines(cell, &mut content);
                        cells.push(content.trim().to_string());
                    }
                    if !cells.is_empty() {
                        rows.push(cells);
                    }
                }
                let columns = rows.iter().map(|row| row.len()).max().unwrap_or(0);
                if columns > 0 {
                    output.push_str(&format!(
                        "#table(\n  columns: {},\n  inset: 4pt,\n  stroke: 0.4pt + rgb(\"#d4d4d8\"),\n",
                        columns
                    ));
                    for row in &rows {
                        output.push_str("  ");
                        for index in 0..columns {
                            output.push('[');
                            output.push_str(row.get(index).map(String::as_str).unwrap_or(""));
                            output.push_str("], ");
                        }
                        output.push('\n');
                    }
                    output.push_str(")\n\n");
                }
            }
            _ => {}
        }
    }
}

fn render_list_item<'a>(
    item: &'a comrak::nodes::AstNode<'a>,
    output: &mut String,
    depth: usize,
) {
    use comrak::nodes::NodeValue;

    for child in item.children() {
        match &child.data.borrow().value {
            NodeValue::Paragraph => {
                render_inlines(child, output);
                output.push('\n');
            }
            NodeValue::List(_) => render_blocks(child, output, depth + 1),
            _ => render_blocks(child, output, depth + 1),
        }
    }
}

fn render_inlines<'a>(
    node: &'a comrak::nodes::AstNode<'a>,
    output: &mut String,
) {
    use comrak::nodes::NodeValue;

    for child in node.children() {
        match &child.data.borrow().value {
            NodeValue::Text(text) | NodeValue::HtmlInline(text) => {
                output.push_str(&escape_typst_text(text));
            }
            NodeValue::Strong => {
                output.push('*');
                render_inlines(child, output);
                output.push('*');
            }
            NodeValue::Emph => {
                output.push('_');
                render_inlines(child, output);
                output.push('_');
            }
            NodeValue::Code(code) => output.push_str(&escape_typst_text(&code.literal)),
            NodeValue::SoftBreak | NodeValue::LineBreak => output.push(' '),
            NodeValue::Link(_) | NodeValue::Image(_) => render_inlines(child, output),
            NodeValue::Strikethrough
            | NodeValue::Superscript
            | NodeValue::Subscript
            | NodeValue::Underline => render_inlines(child, output),
            _ => render_inlines(child, output),
        }
    }
}

/// Normalize model-written resume markdown into structures the Typst renderer can lay out.
///
/// Models frequently emit literal bullet glyphs, whole-line bold pseudo-headings and
/// category-prefixed bullets. Left alone these render as flat paragraphs of punctuation.
pub(crate) fn polish_resume_markdown(markdown: &str) -> String {
    use std::collections::HashSet;

    let bold_only = regex::Regex::new(r"^\*\*([^*]+)\*\*$").unwrap();
    let bold_label = regex::Regex::new(r"^\*\*([^*:]{2,80}):\*\*\s*").unwrap();
    let year_range = regex::Regex::new(
        r"((19|20)\d{2}|\d{1,2}/(19|20)\d{2})\s*[-\u{2013}\u{2014}]\s*((19|20)\d{2}|\d{1,2}/(19|20)\d{2}|[Pp]resent)",
    )
    .unwrap();
    let bullet_glyph = regex::Regex::new(r"^[\u{2022}*]\s+").unwrap();

    const SECTIONS: &[&str] = &[
        "summary",
        "professional summary",
        "profile",
        "core competencies",
        "competencies",
        "technical skills",
        "skills",
        "experience",
        "professional experience",
        "work experience",
        "education",
        "certifications",
        "certifications & key achievements",
        "key achievements",
        "military service & education",
        "military service",
        "projects",
        "awards",
        "publications",
    ];

    let mut output: Vec<String> = Vec::new();
    let mut seen_bullets: HashSet<String> = HashSet::new();
    let mut first_content = true;

    for raw in markdown.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            output.push(String::new());
            continue;
        }

        // Literal bullet glyphs become real Markdown list items.
        let mut line = bullet_glyph.replace(trimmed, "- ").to_string();

        // Whole-line bold pseudo-headings become headings.
        if let Some(captures) = bold_only.captures(&line) {
            let text = captures.get(1).unwrap().as_str().trim();
            let lower = text.to_lowercase();
            let letters = text.chars().filter(|character| character.is_alphabetic()).count().max(1);
            let uppercase = text.chars().filter(|character| character.is_uppercase()).count();
            let uppercase_ratio = uppercase as f32 / letters as f32;
            let heading = if first_content {
                Some(format!("# {}", text))
            } else if year_range.is_match(text) {
                Some(format!("### {}", text))
            } else if SECTIONS.contains(&lower.as_str()) || uppercase_ratio > 0.6 {
                Some(format!("## {}", text))
            } else {
                None
            };
            if let Some(heading) = heading {
                output.push(heading);
                first_content = false;
                continue;
            }
        }

        if let Some(rest) = line.strip_prefix("- ") {
            // Category labels bolted onto bullets read as keyword stuffing.
            let stripped = bold_label.replace(rest, "").to_string();
            if !stripped.trim().is_empty() {
                line = format!("- {}", stripped.trim());
            }

            // Identical sentences under two employers read as fabrication; keep the first.
            let key: String = line[2..]
                .to_lowercase()
                .chars()
                .map(|character| {
                    if character.is_alphanumeric() || character == ' ' {
                        character
                    } else {
                        ' '
                    }
                })
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if key.len() >= 25 && !seen_bullets.insert(key) {
                continue;
            }
        }

        first_content = false;
        output.push(line);
    }

    let mut polished = output.join("\n");
    while polished.contains("\n\n\n") {
        polished = polished.replace("\n\n\n", "\n\n");
    }
    polished.trim().to_string()
}

fn escape_typst_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if matches!(character, '#' | '$' | '%' | '&' | '_' | '*' | '@' | '<' | '>' | '[' | ']' | '\\' | '`') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn ensure_document_font_defaults(path: &Path) -> Result<(), String> {
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("Failed to read Typst source: {}", error))?;
    if !source.starts_with(DOCUMENT_MARKER) {
        std::fs::write(path, with_document_font_defaults(&source))
            .map_err(|error| format!("Failed to set Typst document fonts: {}", error))?;
    }
    Ok(())
}

/// Resolve a bundled sidecar, with a source-tree fallback for development builds.
pub(crate) fn sidecar_path(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let executable = if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    };
    let relative_path = Path::new("sidecars").join(executable);
    let resource_path = app
        .path()
        .resolve(&relative_path, BaseDirectory::Resource);

    if let Ok(path) = &resource_path {
        if path.is_file() {
            return Ok(path.clone());
        }
    }

    let development_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&relative_path);
    if development_path.is_file() {
        return Ok(development_path);
    }

    let resource_error = resource_path
        .err()
        .map(|error| format!("; resource lookup failed: {}", error))
        .unwrap_or_default();
    Err(format!(
        "Bundled {} executable not found. Checked {}{} and {}",
        name,
        relative_path.display(),
        resource_error,
        development_path.display()
    ))
}

pub(crate) fn sidecar_command(
    app: &AppHandle,
    name: &str,
    working_directory: &Path,
) -> Result<(PathBuf, Command), String> {
    let executable_path = sidecar_path(app, name)?;
    let working_directory = working_directory
        .canonicalize()
        .map_err(|error| format!("Failed to resolve sidecar working directory: {}", error))?;
    let mut command = Command::new(&executable_path);
    command.current_dir(&working_directory);
    Ok((executable_path, command))
}

/// Compile a .typ file to PDF using bundled Typst binary.
#[tauri::command]
pub fn compile_typst_pdf(
    typst_file_path: String,
    app: AppHandle,
) -> Result<CompileResult, String> {
    let input_path = Path::new(&typst_file_path);
    if !input_path.exists() {
        return Err(format!("Typst file not found: {}", typst_file_path));
    }

    let input_path = input_path
        .canonicalize()
        .map_err(|error| format!("Failed to resolve Typst file path: {}", error))?;
    ensure_document_font_defaults(&input_path)?;
    let output_path = input_path.with_extension("pdf");

    let working_directory = input_path
        .parent()
        .ok_or("Typst file has no parent directory")?;
    let (typst_path, mut command) = sidecar_command(&app, "typst", working_directory)?;
    let output = command
        .arg("compile")
        .arg(&input_path)
        .arg(&output_path)
        .output()
        .map_err(|error| format!(
            "Failed to execute bundled Typst binary at {} from {}: {}",
            typst_path.display(),
            working_directory.display(),
            error
        ))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Ok(CompileResult {
            pdf_path: output_path.to_string_lossy().into_owned(),
            success: false,
            error_message: Some(format!("Typst Compilation Failed:\n{}", stderr)),
        });
    }

    Ok(CompileResult {
        pdf_path: output_path.to_string_lossy().into_owned(),
        success: true,
        error_message: None,
    })
}

/// Rasterize every PDF page to PNG for visual review gate.
#[tauri::command]
pub fn rasterize_pdf_pages(
    pdf_path: String,
    out_dir: String,
    app: AppHandle,
) -> Result<RasterizeResult, String> {
    let pdf = Path::new(&pdf_path);
    if !pdf.exists() {
        return Err(format!("PDF file not found: {}", pdf_path));
    }

    let out = Path::new(&out_dir);
    if !out.exists() {
        std::fs::create_dir_all(out).map_err(|e| format!("Failed to create output dir: {}", e))?;
    }

    let pdf = pdf
        .canonicalize()
        .map_err(|error| format!("Failed to resolve PDF path: {}", error))?;
    let out = out
        .canonicalize()
        .map_err(|error| format!("Failed to resolve raster output directory: {}", error))?;
    let (pdftoppm_path, mut command) = sidecar_command(&app, "pdftoppm", &out)?;
    let output = command
        .arg("-png")
        .arg("-r")
        .arg("150")
        .arg(&pdf)
        .arg(out.join("page"))
        .output()
        .map_err(|error| format!(
            "Failed to execute bundled pdftoppm at {} from {}: {}",
            pdftoppm_path.display(),
            out.display(),
            error
        ))?;

    if !output.status.success() {
        return Err(format!("pdftoppm failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    // Count generated PNG files
    let mut png_paths: Vec<String> = Vec::new();
    let mut page_count: u32 = 0;

    if let Ok(entries) = std::fs::read_dir(out) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "png").unwrap_or(false) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("page-") || name.starts_with("page") {
                        png_paths.push(path.to_string_lossy().into_owned());
                        page_count += 1;
                    }
                }
            }
        }
    }

    png_paths.sort();

    Ok(RasterizeResult {
        png_paths,
        page_count,
        success: true,
        error_message: None,
    })
}

/// Run layout analysis on rendered PDF pages to detect visual flags.
#[tauri::command]
pub fn analyze_visual_qa(
    pdf_path: String,
    png_paths: Vec<String>,
    employer_name: Option<String>,
    job_title: Option<String>,
    app: AppHandle,
) -> Result<VisualFlags, String> {
    let mut flags = VisualFlags {
        overflow_flags: Vec::new(),
        orphan_heading_flags: Vec::new(),
        broken_wrap_flags: Vec::new(),
        placeholder_flags: Vec::new(),
        metadata_flags: Vec::new(),
    };

    // Analyze each PNG page for visual issues
    for (i, _png_path) in png_paths.iter().enumerate() {
        let page_num = i + 1;

        // Check for placeholder text patterns in the page (via OCR or text layer)
        // Since we don't have OCR integrated here, check the PDF text layer
        let _page_flags = check_page_content(
            pdf_path.as_str(),
            page_num,
            employer_name.as_deref(),
            job_title.as_deref(),
            &app,
        )?;

        // Merge flags
        flags.placeholder_flags.extend(_page_flags.0);
        flags.metadata_flags.extend(_page_flags.1);
    }

    // Check for orphan headings and broken wraps based on page content analysis
    // These are heuristic checks; full implementation would use pdftotext -bbox
    let page_count = png_paths.len();
    // Two-page resumes are expected; three or more usually means content spilled badly.
    if page_count > 2 {
        flags.orphan_heading_flags.push(format!(
            "Document spans {} pages - verify the final page is not an orphan",
            page_count
        ));
    }

    Ok(flags)
}

/// Check individual page content for placeholders and metadata issues.
fn check_page_content(
    pdf_path: &str,
    page_num: usize,
    employer_name: Option<&str>,
    job_title: Option<&str>,
    app: &AppHandle,
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut placeholder_flags = Vec::new();
    let mut metadata_flags = Vec::new();

    // Use pdftotext to extract text from specific page for placeholder detection
    let pdf_path = Path::new(pdf_path)
        .canonicalize()
        .map_err(|error| format!("Failed to resolve PDF path: {}", error))?;
    let working_directory = pdf_path
        .parent()
        .ok_or("PDF file has no parent directory")?;
    let (pdftotext_path, mut command) =
        sidecar_command(app, "pdftotext", working_directory)?;
    let text_output = command
        .arg("-f")
        .arg(page_num.to_string())
        .arg("-l")
        .arg(page_num.to_string())
        .arg(&pdf_path)
        .arg("-")
        .output()
        .map_err(|error| format!(
            "Failed to execute bundled pdftotext at {} from {}: {}",
            pdftotext_path.display(),
            working_directory.display(),
            error
        ))?;

    let page_text = String::from_utf8_lossy(&text_output.stdout);
    let page_lower = page_text.to_lowercase();

    // Placeholder detection
    let placeholders = [
        "[insert name]", "[insert employer]", "[insert title]",
        "[company]", "[role]", "[date]", "[your name]",
        "todo", "fixme", "placeholder", "xxx",
        "lorem ipsum",
    ];

    for ph in &placeholders {
        if page_lower.contains(ph) {
            placeholder_flags.push(format!(
                "Page {}: placeholder text found: '{}'", page_num, ph
            ));
        }
    }

    // Metadata alignment
    if let Some(emp) = employer_name {
        if !page_lower.contains(&emp.to_lowercase()) {
            metadata_flags.push(format!(
                "Page {}: employer '{}' not found on this page", page_num, emp
            ));
        }
    }
    if let Some(title) = job_title {
        if !page_lower.contains(&title.to_lowercase()) {
            metadata_flags.push(format!(
                "Page {}: job title '{}' not found on this page", page_num, title
            ));
        }
    }

    Ok((placeholder_flags, metadata_flags))
}

/// Verify all bundled sidecar binaries are present and executable.
#[tauri::command]
pub fn verify_sidecars(app: AppHandle) -> Result<Vec<String>, String> {
    let mut results = Vec::new();
    let binaries = vec!["typst", "pdftoppm", "pdftotext"];

    for binary in &binaries {
        match sidecar_path(&app, binary) {
            Ok(path) => {
                let working_directory = path.parent().unwrap_or_else(|| Path::new("."));
                let output = Command::new(&path)
                    .current_dir(working_directory)
                    .arg("--version")
                    .output();
                match output {
                    Ok(out) if out.status.success() => {
                        let version = String::from_utf8_lossy(&out.stdout);
                        results.push(format!("✓ {}: {} ({})", binary, path.display(), version.trim()));
                    }
                    _ => {
                        results.push(format!("⚠ {}: exists but version check failed ({})", binary, path.display()));
                    }
                }
            }
            Err(error) => results.push(format!("✗ {}: {}", binary, error)),
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::{markdown_to_typst, with_document_font_defaults, DOCUMENT_MARKER};

    #[test]
    fn markdown_documents_use_typst_layout_and_segoe_ui_first() {
        let source = "# CareerOS\n\nA **clear** and *polished* letter.\n\n- First item\n- Second item\n";
        let styled = with_document_font_defaults(source);

        assert!(styled.starts_with(DOCUMENT_MARKER));
        assert!(styled.contains("#set text(font: (\"Segoe UI\", \"Arial\", \"Liberation Sans\"), size: 10pt)"));
        assert!(styled.contains("= CareerOS"));
        assert!(styled.contains("*clear*"));
        assert!(styled.contains("_polished_"));
        assert!(styled.contains("- First item"));
        assert!(!styled.contains("**"));
        assert_eq!(with_document_font_defaults(&styled), styled);
    }

    #[test]
    fn markdown_text_escapes_typst_markup_characters() {
        let rendered = markdown_to_typst("A #tag with 50% & [brackets].");
        assert!(rendered.contains(r"A \#tag with 50\% \& \[brackets\]."));
    }
}
#[cfg(test)]
mod polish_tests {
    use super::{markdown_to_typst, polish_resume_markdown};

    #[test]
    fn tables_render_as_typst_tables_instead_of_pipe_text() {
        let markdown = "| A | B |\n| --- | --- |\n| one | two |\n";
        let typst = markdown_to_typst(markdown);
        assert!(typst.contains("#table("));
        assert!(!typst.contains("| ---"));
    }

    #[test]
    fn resume_polish_normalizes_headings_bullets_and_duplicate_lines() {
        let markdown = "**JANE DOE**\n\n**PROFESSIONAL EXPERIENCE**\n\n\u{2022} **Category Label:** Designed and maintained client-facing manuals for five regions.\n\n**Second Role (09/2020 \u{2013} 09/2022)**\n\n\u{2022} **Other Label:** Designed and maintained client-facing manuals for five regions.\n\n\u{2022} Taught instructor-led sessions for clinical staff.\n";
        let polished = polish_resume_markdown(markdown);
        assert!(polished.starts_with("# JANE DOE"));
        assert!(polished.contains("## PROFESSIONAL EXPERIENCE"));
        assert!(polished.contains("### Second Role (09/2020 \u{2013} 09/2022)"));
        assert!(polished.contains("- Designed and maintained client-facing manuals for five regions."));
        assert_eq!(polished.matches("Designed and maintained client-facing manuals").count(), 1);
        assert!(polished.contains("- Taught instructor-led sessions"));
    }
}
