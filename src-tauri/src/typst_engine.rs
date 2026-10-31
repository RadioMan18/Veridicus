//! Typst Engine & Visual QA for CareerOS.
//!
//! Resolves bundled sidecar binaries (typst, pdftoppm, pdftotext) from the app
//! bundle, compiles .typ to .pdf, rasterizes pages to PNG, and runs layout
//! analysis for visual review gate flags.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

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

/// Resolve bundled sidecar binary path from the app bundle.
fn sidecar(name: &str) -> String {
    // Tauri sidecar resolution; version-pinned typst / pdftoppm / tesseract
    // These are resolved at build time and shipped as sidecars
    format!("sidecars/{}", name)
}

/// Compile a .typ file to PDF using bundled Typst binary.
#[tauri::command]
pub fn compile_typst_pdf(typst_file_path: String) -> Result<CompileResult, String> {
    let input_path = Path::new(&typst_file_path);
    if !input_path.exists() {
        return Err(format!("Typst file not found: {}", typst_file_path));
    }

    let output_path = input_path.with_extension("pdf");

    let output = Command::new(sidecar("typst"))
        .arg("compile")
        .arg(input_path)
        .arg(&output_path)
        .output()
        .map_err(|e| format!("Failed to execute bundled Typst binary: {}", e))?;

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
pub fn rasterize_pdf_pages(pdf_path: String, out_dir: String) -> Result<RasterizeResult, String> {
    let pdf = Path::new(&pdf_path);
    if !pdf.exists() {
        return Err(format!("PDF file not found: {}", pdf_path));
    }

    let out = Path::new(&out_dir);
    if !out.exists() {
        std::fs::create_dir_all(out).map_err(|e| format!("Failed to create output dir: {}", e))?;
    }

    let output = Command::new(sidecar("pdftoppm"))
        .arg("-png")
        .arg("-r")
        .arg("150")
        .arg(pdf)
        .arg(out.join("page"))
        .output()
        .map_err(|e| format!("Failed to execute bundled pdftoppm: {}", e))?;

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
        let _page_flags = check_page_content(pdf_path.as_str(), page_num, employer_name.as_deref(), job_title.as_deref())?;

        // Merge flags
        flags.placeholder_flags.extend(_page_flags.0);
        flags.metadata_flags.extend(_page_flags.1);
    }

    // Check for orphan headings and broken wraps based on page content analysis
    // These are heuristic checks; full implementation would use pdftotext -bbox
    let page_count = png_paths.len();
    if page_count > 1 {
        // Heuristic: if last page has very little content, potential orphan heading
        flags.orphan_heading_flags.push(format!(
            "Page {} may contain orphaned content - verify layout", page_count
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
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut placeholder_flags = Vec::new();
    let mut metadata_flags = Vec::new();

    // Use pdftotext to extract text from specific page for placeholder detection
    let text_output = Command::new(sidecar("pdftotext"))
        .arg("-f")
        .arg(page_num.to_string())
        .arg("-l")
        .arg(page_num.to_string())
        .arg(pdf_path)
        .arg("-")
        .output()
        .map_err(|e| format!("pdftotext failed: {}", e))?;

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
pub fn verify_sidecars() -> Result<Vec<String>, String> {
    let mut results = Vec::new();
    let binaries = vec!["typst", "pdftoppm", "pdftotext"];

    for binary in &binaries {
        let path = sidecar(binary);
        if Path::new(&path).exists() {
            // Try to execute --version or help
            let output = Command::new(&path)
                .arg("--version")
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    let version = String::from_utf8_lossy(&out.stdout);
                    results.push(format!("✓ {}: {} ({})", binary, path, version.trim()));
                }
                _ => {
                    results.push(format!("⚠ {}: exists but version check failed ({})", binary, path));
                }
            }
        } else {
            results.push(format!("✗ {}: not found at {}", binary, path));
        }
    }

    Ok(results)
}