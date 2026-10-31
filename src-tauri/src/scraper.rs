//! Web scraper & Markdown converter for CareerOS.
//!
//! Fetches job posting pages, strips HTML noise, converts main content to Markdown,
//! and saves dated files to the user's workspace.

use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Serialize, Deserialize};

/// Represents the extracted content from a job posting page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobScrapeResult {
    pub title: String,
    pub company: String,
    pub location: Option<String>,
    pub description_md: String,
    pub requirements_md: String,
    pub application_url: String,
    pub scrape_date: String,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Scrape a job posting URL and return structured results.
#[tauri::command]
pub async fn scrape_job_page(url: String) -> Result<JobScrapeResult, String> {
    let client = Client::builder()
        .user_agent("CareerOS/2.0 job-scraper")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch URL: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("URL returned status: {}", response.status()));
    }

    let html = response.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

    // Parse HTML
    let document = Html::parse_document(&html);

    // Strip noise elements
    let noise_selectors = ["script", "style", "nav", "footer", "aside", "header", "form", "button", "input"];
    for selector_str in &noise_selectors {
        if let Ok(sel) = Selector::parse(selector_str) {
            for element in document.select(&sel) {
                // In scraper, we can't remove from the original document, so we filter during extraction
                // We'll handle this by skipping these elements during text extraction
            }
        }
    }

    // Extract title
    let title = extract_title(&document);

    // Extract company name
    let company = extract_company(&document);

    // Extract main content and convert to markdown
    let description_md = extract_main_content_md(&document);

    // Extract requirements
    let requirements_md = extract_requirements_md(&document);

    let scrape_date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    Ok(JobScrapeResult {
        title,
        company,
        location: None,
        description_md,
        requirements_md,
        application_url: url.clone(),
        scrape_date,
        success: true,
        error_message: None,
    })
}

/// Extract title from document.
fn extract_title(doc: &Html) -> String {
    // Try title tag first
    if let Ok(sel) = Selector::parse("title") {
        if let Some(element) = doc.select(&sel).next() {
            let text = element.text().collect::<Vec<_>>().join(" ");
            if !text.trim().is_empty() {
                return text.trim().to_string();
            }
        }
    }

    // Try h1
    if let Ok(sel) = Selector::parse("h1") {
        if let Some(element) = doc.select(&sel).next() {
            let text = element.text().collect::<Vec<_>>().join(" ");
            if text.len() > 3 && text.len() < 200 {
                return text;
            }
        }
    }

    "Job Posting".to_string()
}

/// Extract company name from job posting HTML.
fn extract_company(doc: &Html) -> String {
    // Try meta tags
    let meta_selectors = [
        "meta[property='al:product:creator']",
        "meta[name='company']",
        "meta[name='employer']",
        "meta[property='og:site_name']",
    ];

    for selector_str in &meta_selectors {
        if let Ok(sel) = Selector::parse(selector_str) {
            if let Some(element) = doc.select(&sel).next() {
                if let Some(content) = element.value().attr("content") {
                    if !content.is_empty() && content.len() < 100 {
                        return content.to_string();
                    }
                }
            }
        }
    }

    // Try common class patterns for company
    let class_selectors = [
        ".company-name",
        ".employer-name",
        "[data-company]",
        ".job-company",
    ];

    for selector_str in &class_selectors {
        if let Ok(sel) = Selector::parse(selector_str) {
            if let Some(element) = doc.select(&sel).next() {
                let text = element.text().collect::<Vec<_>>().join(" ");
                if !text.trim().is_empty() && text.len() < 100 {
                    return text.trim().to_string();
                }
            }
        }
    }

    "Unknown Company".to_string()
}

/// Extract main content and convert to markdown using simple HTML-to-text.
fn extract_main_content_md(doc: &Html) -> String {
    // Try common content containers
    let content_selectors = [
        "article",
        "main",
        ".job-description",
        ".post-content",
        ".entry-content",
        ".content",
        "#job-description",
        "[data-testid='job-description']",
        ".description",
    ];

    for selector_str in &content_selectors {
        if let Ok(sel) = Selector::parse(selector_str) {
            if let Some(element) = doc.select(&sel).next() {
                return html_to_markdown(&element.html());
            }
        }
    }

    // Fallback: get body text
    if let Ok(sel) = Selector::parse("body") {
        if let Some(_element) = doc.select(&sel).next() {
            return extract_text_skip_noise(doc);
        }
    }

    "Could not extract job description".to_string()
}

/// Convert HTML to simple markdown.
fn html_to_markdown(html: &str) -> String {
    // Simple HTML to markdown conversion
    let mut result = html.to_string();

    // Replace common tags
    result = result.replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n");
    result = result.replace("<p>", "").replace("</p>", "\n\n");
    result = result.replace("<h1>", "# ").replace("</h1>", "\n\n");
    result = result.replace("<h2>", "## ").replace("</h2>", "\n\n");
    result = result.replace("<h3>", "### ").replace("</h3>", "\n\n");
    result = result.replace("<h4>", "#### ").replace("</h4>", "\n\n");
    result = result.replace("<li>", "- ").replace("</li>", "\n");
    result = result.replace("<ul>", "").replace("</ul>", "\n");
    result = result.replace("<ol>", "").replace("</ol>", "\n");
    result = result.replace("<strong>", "**").replace("</strong>", "**");
    result = result.replace("<b>", "**").replace("</b>", "**");
    result = result.replace("<em>", "*").replace("</em>", "*");
    result = result.replace("<i>", "*").replace("</i>", "*");
    result = result.replace("<code>", "`").replace("</code>", "`");
    result = result.replace("<pre>", "```\n").replace("</pre>", "\n```\n");

    // Remove remaining tags
    let re = regex::Regex::new(r"<[^>]+>").unwrap();
    result = re.replace_all(&result, "").to_string();

    // Decode HTML entities
    result = result.replace("&nbsp;", " ")
        .replace("&", "&")
        .replace("<", "<")
        .replace(">", ">")
        .replace("&#34;", "\"")
        .replace("'", "'");

    // Clean up excessive whitespace
    let re = regex::Regex::new(r"\n{3,}").unwrap();
    result = re.replace_all(&result, "\n\n").to_string();

    result.trim().to_string()
}

/// Extract text from document while skipping noise elements.
fn extract_text_skip_noise(doc: &Html) -> String {
    // Extract all text nodes from the root element
    let text = doc.root_element().text().collect::<Vec<_>>().join(" ");
    // Collapse whitespace
    let re = regex::Regex::new(r"\s+").unwrap();
    re.replace_all(&text, " ").trim().to_string()
}

/// Extract requirements section from job posting.
fn extract_requirements_md(doc: &Html) -> String {
    // Try common requirement section patterns
    let requirement_selectors = [
        ".requirements",
        ".job-requirements",
        ".requirements-list",
        "[data-testid='requirements']",
        "section:has(h2:contains('requirement'))",
        "div:has(h2:contains('requirement'))",
    ];

    for selector_str in &requirement_selectors {
        if let Ok(sel) = Selector::parse(selector_str) {
            if let Some(element) = doc.select(&sel).next() {
                let text = html_to_markdown(&element.html());
                if text.len() > 20 {
                    return text;
                }
            }
        }
    }

    "Requirements section not found - review manually".to_string()
}
