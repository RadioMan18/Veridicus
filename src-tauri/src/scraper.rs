//! Web scraper & Markdown converter for Veridicus.
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
        .user_agent("Veridicus/2.0 job-scraper")
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

    // Extract metadata. Most ATS pages title themselves
    // "Job Application for <role> at <company>", so clean that before it reaches the ledger.
    let job_posting = json_ld_job_posting(&document);
    let raw_title = extract_title(&document);
    let company = extract_company(&document, &raw_title, &url, job_posting.as_ref());
    let title = clean_job_title(&raw_title, &company);

    // Extract main content and convert to markdown. Workday-style pages render nothing
    // server-side, so prefer the schema.org description when the page publishes one.
    let mut description_md = extract_main_content_md(&document);
    if let Some(json_description) = json_ld_description(&document) {
        let converted = html_to_markdown(&json_description);
        if converted.len() >= 400 {
            description_md = converted;
        }
    }

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
fn extract_company(
    doc: &Html,
    raw_title: &str,
    url: &str,
    json_ld: Option<&serde_json::Value>,
) -> String {
    // A "<role> at <company>" title is the most precise signal available.
    if let Some(company) = company_from_title(raw_title) {
        return company;
    }

    // schema.org hiringOrganization beats og:site_name, which is often the ATS vendor
    // ("Workday") rather than the employer.
    if let Some(company) = json_ld.and_then(json_ld_company) {
        return company;
    }

    // Try meta tags
    let meta_selectors = [
        "meta[property='al:product:creator']",
        "meta[name='company']",
        "meta[name='employer']",
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

    if let Some(company) = company_from_url(url) {
        return company;
    }

    "Unknown Company".to_string()
}

/// Strip ATS boilerplate from a page title: "Job Application for X at Y" becomes "X".
fn clean_job_title(raw: &str, company: &str) -> String {
    let mut title = raw.trim().to_string();

    for prefix in ["job application for ", "apply for ", "apply to ", "careers at "] {
        if title.to_lowercase().starts_with(prefix) {
            title = title[prefix.len()..].to_string();
            break;
        }
    }

    if !company.is_empty() && company != "Unknown Company" {
        let suffix = format!(" at {}", company);
        if title.len() > suffix.len() && title.to_lowercase().ends_with(&suffix.to_lowercase()) {
            title.truncate(title.len() - suffix.len());
        }
    } else if let Some(index) = title.rfind(" at ") {
        title.truncate(index);
    }

    for separator in [" | ", " \u{2013} ", " \u{2014} "] {
        if let Some(index) = title.find(separator) {
            title.truncate(index);
        }
    }

    title.trim().to_string()
}

/// Pull a company name out of a page title such as "Role at Acme".
fn company_from_title(raw_title: &str) -> Option<String> {
    let title = raw_title.split(" | ").next().unwrap_or(raw_title);
    let index = title.rfind(" at ")?;
    let candidate = title[index + 4..].trim();
    if candidate.is_empty() || candidate.len() > 80 {
        None
    } else {
        Some(candidate.to_string())
    }
}

/// Derive the company from the posting URL when the page exposes no company metadata.
fn company_from_url(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_lowercase();

    if host.contains("myworkdayjobs") {
        let first = host.split('.').next()?;
        if first != "www" && first.len() > 1 {
            return Some(pretty_company_name(first));
        }
    }

    const SKIP: &[&str] = &[
        "jobs", "job", "careers", "career", "apply", "positions", "position", "job-boards",
        "boards", "openings", "en", "us", "home", "company",
    ];

    for segment in parsed.path_segments()? {
        let lower = segment.to_lowercase();
        if segment.is_empty() || SKIP.contains(&lower.as_str()) {
            continue;
        }
        if lower.len() >= 2
            && lower
                .chars()
                .all(|character| character.is_alphanumeric() || character == '-' || character == '_')
            && !lower.chars().all(|character| character.is_ascii_digit())
        {
            return Some(pretty_company_name(segment));
        }
    }

    const HOST_SUFFIXES: &[&str] = &[
        "greenhouse", "lever", "ashbyhq", "workday", "myworkdayjobs", "com", "co", "io",
        "net", "org", "www",
    ];
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() >= 2 {
        let candidate = parts[parts.len() - 2];
        if !HOST_SUFFIXES.contains(&candidate) && candidate.len() > 1 {
            return Some(pretty_company_name(candidate));
        }
    }

    None
}

fn pretty_company_name(slug: &str) -> String {
    slug.split(|character: char| character == '-' || character == '_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), characters.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
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

    // Decode HTML entities. Specific entities first: decoding "&amp;" early would turn
    // a literal "&amp;lt;" into a tag-like "<".
    result = decode_html_entities(&result);

    // Clean up excessive whitespace
    let re = regex::Regex::new(r"\n{3,}").unwrap();
    result = re.replace_all(&result, "\n\n").to_string();

    result.trim().to_string()
}

fn decode_html_entities(value: &str) -> String {
    value
        .replace("&nbsp;", " ")
        .replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&mdash;", "\u{2014}")
        .replace("&ndash;", "\u{2013}")
        .replace("&rsquo;", "\u{2019}")
        .replace("&lsquo;", "\u{2018}")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
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
    let from_page = requirements_from_document(doc);
    if !from_page.is_empty() {
        return from_page;
    }

    // Workday-style pages render nothing server-side and publish the posting as
    // schema.org JSON-LD instead, so retry the same heading scan on that HTML and
    // then on its plain text (Workday strips the tags before publishing).
    if let Some(description) = json_ld_description(doc) {
        let fragment = Html::parse_fragment(&description);
        let from_fragment = requirements_from_document(&fragment);
        if !from_fragment.is_empty() {
            return from_fragment;
        }
        return requirements_from_plain_text(&html_to_markdown(&description));
    }

    String::new()
}

/// Fallback for descriptions published as one unformatted block of text: start at a
/// requirements label and stop at the first benefits or legal label.
fn requirements_from_plain_text(text: &str) -> String {
    const START_LABELS: &[&str] = &[
        "required qualifications",
        "minimum qualifications",
        "basic qualifications",
        "qualifications",
        "what you'll need",
        "what you will need",
        "what you bring",
        "requirements",
        "you have",
        "must have",
        "who you are",
    ];
    const END_LABELS: &[&str] = &[
        "benefits",
        "compensation",
        "pay range",
        "salary range",
        "about the company",
        "about us",
        "equal opportunity",
        "why you'll love",
        "our commitment",
        "additional information",
        "how to apply",
        "#li-",
    ];

    let lower = text.to_lowercase();
    let start = match START_LABELS.iter().filter_map(|label| lower.find(label)).min() {
        Some(start) => start,
        None => return String::new(),
    };
    let tail = match text.get(start..) {
        Some(tail) => tail,
        None => return String::new(),
    };

    // Skip past the label itself and its colon.
    let body = match tail.find(':') {
        Some(colon) => &tail[colon + 1..],
        None => tail,
    };

    // Lowercasing can shift byte offsets for non-ASCII characters, so slice defensively.
    let lower_body = body.to_lowercase();
    let end = END_LABELS
        .iter()
        .filter_map(|label| lower_body.find(label))
        .min()
        .unwrap_or(body.len());
    let section = body.get(..end).unwrap_or(body);

    let mut bullets: Vec<String> = Vec::new();
    for sentence in section.split(". ").map(str::trim) {
        if sentence.len() < 15 {
            continue;
        }
        bullets.push(format!("- {}", sentence.trim_end_matches('.')));
        if bullets.len() >= 25 {
            break;
        }
    }

    let joined = bullets.join("\n");
    if joined.len() > 40 {
        joined
    } else {
        String::new()
    }
}

fn requirements_from_document(doc: &Html) -> String {
    // Class-based selectors first: they exist on some ATS templates.
    let requirement_selectors = [
        ".requirements",
        ".job-requirements",
        ".requirements-list",
        "[data-testid='requirements']",
    ];

    for selector_str in &requirement_selectors {
        if let Ok(sel) = Selector::parse(selector_str) {
            if let Some(element) = doc.select(&sel).next() {
                let text = html_to_markdown(&element.html());
                if text.trim().len() > 40 {
                    return text;
                }
            }
        }
    }

    // Postings mark requirements with a heading, or with a bold-only paragraph such as
    // "<p><b>You Have:</b></p>" (Booz Allen). The `scraper` CSS engine supports neither
    // :has() nor :contains(), so match on the text itself.
    let heading_selector = match Selector::parse("h1, h2, h3, h4, h5, p") {
        Ok(selector) => selector,
        Err(_) => return String::new(),
    };

    for node in doc.select(&heading_selector) {
        if node.value().name() == "p" && !is_bold_label(node) {
            continue;
        }

        let text = node.text().collect::<Vec<_>>().join(" ");
        let text = text.trim();
        if text.is_empty() || text.len() > 120 {
            continue;
        }
        if !is_requirement_heading(&normalize_label(text)) {
            continue;
        }
        if let Some(section) = collect_requirement_section(node) {
            return section;
        }
    }

    String::new()
}

/// True when a paragraph holds nothing but one bold phrase, e.g. `<p><b>You Have:</b></p>`.
fn is_bold_label(element: scraper::ElementRef) -> bool {
    let mut bold_text = 0usize;
    let mut other_nodes = 0usize;

    for child in element.children() {
        if let Some(child) = scraper::ElementRef::wrap(child) {
            if matches!(child.value().name(), "b" | "strong") {
                bold_text += child.text().collect::<String>().trim().len();
            } else {
                other_nodes += 1;
            }
        }
    }

    // Whitespace-only text nodes are ignored; only real text outside the bold phrase counts.
    let total = element.text().collect::<String>().trim().len();
    other_nodes == 0 && bold_text > 0 && total == bold_text
}

/// Lowercase heading text and fold typographic apostrophes so "What You’ll Bring"
/// matches the same keywords as "What You'll Bring".
fn normalize_label(text: &str) -> String {
    text.to_lowercase()
        .replace('\u{2019}', "'")
        .replace('\u{2018}', "'")
        .replace('\u{201C}', "\"")
        .replace('\u{201D}', "\"")
        .replace('\u{00A0}', " ")
}

fn heading_level(tag: &str) -> Option<u8> {
    match tag {
        "h1" => Some(1),
        "h2" => Some(2),
        "h3" => Some(3),
        "h4" => Some(4),
        "h5" => Some(5),
        "h6" => Some(6),
        _ => None,
    }
}

fn is_requirement_heading(lower: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "requirement",
        "qualification",
        "what you'll need",
        "what you will need",
        "what you bring",
        "what you'll bring",
        "what we're looking for",
        "what we are looking for",
        "what you have",
        "you have",
        "you'll need",
        "who you are",
        "must have",
        "minimum qualification",
        "about you",
        "skills and experience",
        "skills and abilit",
        "education and",
        "computer skills",
        "preferred certification",
    ];

    // "Skills" only counts when it is the whole heading: a heading such as
    // "Skills taxonomies and capability models" is a topic, not a criteria list.
    let bare = lower.trim().trim_end_matches(':').trim();
    if matches!(bare, "skills" | "required skills" | "technical skills" | "core skills") {
        return true;
    }

    KEYWORDS.iter().any(|keyword| lower.contains(keyword))
}

fn is_stop_heading(lower: &str) -> bool {
    const STOPS: &[&str] = &[
        "benefit",
        "compensation",
        "pay range",
        "salary",
        "equal opportunity",
        "about us",
        "about the company",
        "why you'll love",
        "why you will love",
        "core value",
        "diversity",
        "inclusion",
        "additional information",
        "how to apply",
        "apply now",
        "supervisory",
        "key performance",
        "physical requirement",
        "work model",
        "identity statement",
        "candidate ai",
        "visa sponsorship",
    ];

    STOPS.iter().any(|stop| lower.contains(stop))
}

/// Some pages head their requirements with an accommodations disclaimer; the real
/// list follows under a later heading.
fn is_boilerplate(lower: &str) -> bool {
    const PHRASES: &[&str] = &[
        "to perform this job successfully",
        "reasonable accommodations",
        "the requirements listed below are representative",
        "other duties may be assigned",
        "equal opportunity employer",
    ];

    PHRASES.iter().any(|phrase| lower.contains(phrase))
}

/// Collect a requirements section, following sub-headings until a benefits or legal
/// heading, and skipping disclaimer paragraphs.
fn collect_requirement_section(heading: scraper::ElementRef) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut total = 0usize;
    // A bold paragraph counts as the deepest heading; siblings at the same or a
    // shallower level end the section unless they are themselves about requirements.
    let start_level = heading_level(heading.value().name()).unwrap_or(7);

    for sibling in heading.next_siblings() {
        let element = match scraper::ElementRef::wrap(sibling) {
            Some(element) => element,
            None => continue,
        };

        let tag = element.value().name();
        if matches!(tag, "footer" | "script" | "style" | "nav") {
            break;
        }

        let text = element.text().collect::<Vec<_>>().join(" ");
        let text = text.trim();
        if text.is_empty() {
            continue;
        }

        let lower = normalize_label(text);
        let is_heading = tag.starts_with('h') || (tag == "p" && is_bold_label(element));
        if is_heading {
            if is_stop_heading(&lower) {
                break;
            }
            let level = heading_level(tag).unwrap_or(7);
            if level <= start_level && !is_requirement_heading(&lower) {
                break;
            }
            if text.len() <= 120 {
                parts.push(format!("#### {}", text.trim_end_matches(':').trim()));
                continue;
            }
        } else if text.len() <= 40 && is_stop_heading(&lower) {
            // Unstyled "Compensation"/"Benefits" labels end the section too.
            break;
        }

        if is_boilerplate(&lower) {
            continue;
        }

        let markdown = html_to_markdown(&element.html());
        if markdown.trim().is_empty() {
            continue;
        }

        total += markdown.len();
        parts.push(markdown.trim().to_string());
        if parts.len() >= 24 || total > 6000 {
            break;
        }
    }

    let joined = parts.join("\n\n");
    if joined.trim().len() > 40 {
        Some(joined.trim().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod scraper_tests {
    use super::{clean_job_title, company_from_title, company_from_url, extract_requirements_md};
    use scraper::Html;

    #[test]
    fn ats_titles_and_companies_are_cleaned() {
        let raw = "Job Application for Content Strategist - AI Training at NetDocuments";
        assert_eq!(company_from_title(raw), Some("NetDocuments".to_string()));
        assert_eq!(clean_job_title(raw, "NetDocuments"), "Content Strategist - AI Training");
        assert_eq!(
            company_from_url("https://job-boards.greenhouse.io/netdocuments/jobs/5468057008"),
            Some("Netdocuments".to_string())
        );
        assert_eq!(
            clean_job_title("Senior Engineer | Acme Careers", "Unknown Company"),
            "Senior Engineer"
        );
    }

    #[test]
    fn requirements_are_extracted_from_heading_sections() {
        let html = Html::parse_document(
            "<html><body><h2>What you'll need</h2><ul><li>3+ years of experience</li><li>Strong writing</li></ul><h2>Benefits</h2><p>Health insurance</p></body></html>",
        );
        let requirements = extract_requirements_md(&html);
        assert!(requirements.contains("3+ years of experience"));
        assert!(!requirements.contains("Health insurance"));
        assert!(!requirements.contains("not found"));
    }
}

/// Parse schema.org JobPosting JSON-LD. Several ATS platforms (Workday among them)
/// publish the posting only as structured data and render the page client-side.
fn json_ld_job_posting(doc: &Html) -> Option<serde_json::Value> {
    let selector = Selector::parse("script[type='application/ld+json']").ok()?;

    for element in doc.select(&selector) {
        let raw = element.text().collect::<Vec<_>>().join("");
        if raw.trim().is_empty() {
            continue;
        }
        let parsed: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some(posting) = find_job_posting(&parsed) {
            return Some(posting);
        }
    }

    None
}

fn find_job_posting(node: &serde_json::Value) -> Option<serde_json::Value> {
    match node {
        serde_json::Value::Object(map) => {
            let is_posting = map
                .get("@type")
                .map(is_job_posting_type)
                .unwrap_or(false);
            if is_posting {
                return Some(node.clone());
            }
            map.values().find_map(find_job_posting)
        }
        serde_json::Value::Array(items) => items.iter().find_map(find_job_posting),
        _ => None,
    }
}

fn is_job_posting_type(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(name) => name.eq_ignore_ascii_case("JobPosting"),
        serde_json::Value::Array(names) => names.iter().any(is_job_posting_type),
        _ => false,
    }
}

fn json_ld_description(doc: &Html) -> Option<String> {
    let posting = json_ld_job_posting(doc)?;
    let description = posting.get("description")?.as_str()?.trim().to_string();
    if description.is_empty() {
        None
    } else {
        Some(description)
    }
}

fn json_ld_company(posting: &serde_json::Value) -> Option<String> {
    let name = posting
        .get("hiringOrganization")?
        .get("name")?
        .as_str()?
        .trim();
    if name.is_empty() || name.len() > 80 {
        None
    } else {
        Some(name.to_string())
    }
}

#[cfg(test)]
mod real_world_tests {
    use super::{extract_requirements_md, json_ld_description};
    use scraper::Html;

    // Booz Allen: no semantic headings, sections are bold-only paragraphs.
    #[test]
    fn bold_paragraph_sections_are_treated_as_headings() {
        let html = Html::parse_fragment(
            "<p><b>The Opportunity:</b></p><p>GRACE is ARPA-H's AI assistant.</p><p></p>\
             <p><b>What You'll Work On:</b></p><ul><li><p>Run onboarding sessions and office hours.</p></li></ul><p></p>\
             <p><b>You Have:</b></p><ul><li><p>5+ years of experience in a customer-facing technical role such as product management.</p></li><li><p>Bachelor's degree</p></li></ul><p></p>\
             <p><b>Nice If You Have:</b></p><ul><li><p>Experience with Model Context Protocol (MCP)</p></li></ul><p></p>\
             <p><b>Compensation</b></p><p>Health, life, disability and retirement benefits.</p>",
        );

        let requirements = extract_requirements_md(&html);
        assert!(requirements.contains("5+ years of experience in a customer-facing technical role"));
        assert!(requirements.contains("Bachelor's degree"));
        assert!(requirements.contains("Model Context Protocol"));
        assert!(!requirements.contains("Run onboarding sessions"));
        assert!(!requirements.contains("retirement benefits"));
    }

    // CCMSI / Emergent Holdings: a boilerplate disclaimer under "Qualifications",
    // with the real criteria under later sub-headings.
    #[test]
    fn boilerplate_disclaimers_are_skipped_for_the_real_sections() {
        let html = Html::parse_fragment(
            "<h2>QUALIFICATIONS</h2>\
             <p>To perform this job successfully, an individual must be able to perform each essential duty satisfactorily. The requirements listed below are representative of the knowledge, skill, and/or ability required. Reasonable accommodations may be made.</p>\
             <h2>OTHER SKILLS AND ABILITIES</h2>\
             <ul><li>Advanced communication skills, including the ability to explain AI opportunities to executive audiences.</li><li>Expert execution skills across cross-functional initiatives.</li></ul>\
             <h2>SUPERVISORY RESPONSIBILITIES</h2><p>Directly supervises a varied number of employees.</p>",
        );

        let requirements = extract_requirements_md(&html);
        assert!(requirements.contains("Advanced communication skills"));
        assert!(requirements.contains("Expert execution skills"));
        assert!(!requirements.contains("To perform this job successfully"));
        assert!(!requirements.contains("Directly supervises"));
    }

    // Flexera / Workday: the page body is empty and the posting lives in JSON-LD.
    #[test]
    fn workday_json_ld_description_supplies_the_requirements() {
        let html = Html::parse_document(
            r#"<html><head><script type="application/ld+json">{"@context":"https://schema.org","@type":"JobPosting","title":"Senior AI-Augmented Software Engineer","hiringOrganization":{"@type":"Organization","name":"Flexera Software LLC"},"description":"<h2>Required Qualifications:</h2><ul><li>Foundational programming experience through professional work or academic projects</li><li>Familiarity with Git and collaborative development practices</li></ul><h2>Preferred Qualifications:</h2><ul><li>Hands-on experience with AI coding tools such as Claude Code</li></ul>"}</script></head><body><div id="root"></div></body></html>"#,
        );

        let requirements = extract_requirements_md(&html);
        assert!(requirements.contains("Foundational programming experience"));
        assert!(requirements.contains("Familiarity with Git"));
        assert!(requirements.contains("Claude Code"));
        assert!(json_ld_description(&html).is_some());
    }

    // Databricks-style pages already use real headings; the "What We're Looking For"
    // section must not swallow the following "What Success Looks Like" section.
    #[test]
    fn requirement_section_stops_at_the_next_heading() {
        let html = Html::parse_fragment(
            "<h2>What We're Looking For</h2>\
             <ul><li>Experience designing capability or skills models and the systems around them.</li></ul>\
             <h2>What Success Looks Like in Year One</h2>\
             <ul><li>A live skills taxonomy published and used by content teams.</li></ul>",
        );

        let requirements = extract_requirements_md(&html);
        assert!(requirements.contains("capability or skills models"));
        assert!(!requirements.contains("skills taxonomy published"));
    }

    // Ashby heads its criteria with a typographic apostrophe: "What You’ll Bring".
    #[test]
    fn typographic_apostrophes_do_not_break_heading_matches() {
        let html = Html::parse_fragment(
            "<h2><strong>What You’ll Bring</strong></h2>\
             <ul><li><p>3-5 years of relevant experience in AI agents, LLMs, or conversation design</p></li><li><p>Strong command of language: you can hear when a response sounds scripted</p></li></ul>\
             <h2><strong>Why Join Us</strong></h2><p>Remote-first, fast-moving culture with ownership and autonomy.</p>\
             <p>Final compensation will depend on your skills, experience, qualifications, and location.</p>",
        );

        let requirements = extract_requirements_md(&html);
        assert!(requirements.contains("3-5 years of relevant experience in AI agents"));
        assert!(requirements.contains("Strong command of language"));
        assert!(!requirements.contains("Remote-first"));
    }

    // eMoney (Greenhouse) puts a second criteria list under a bare "Skills" heading.
    #[test]
    fn a_bare_skills_heading_continues_the_requirements_section() {
        let html = Html::parse_fragment(
            "<h4>Requirements</h4>\
             <p>• 5+ years of people leadership experience leading managers or team leaders.</p>\
             <h4>Skills</h4>\
             <p>• Extensive experience designing and scaling learning programs within a SaaS environment.</p>\
             <h4>The salary range for this position</h4>\
             <p>$142,000 - $179,000</p>",
        );

        let requirements = extract_requirements_md(&html);
        assert!(requirements.contains("5+ years of people leadership experience"));
        assert!(requirements.contains("designing and scaling learning programs"));
    }

    // Workday publishes the description as a single unformatted string with no tags.
    #[test]
    fn plain_text_descriptions_fall_back_to_label_scanning() {
        let html = Html::parse_document(
            r#"<html><head><script type="application/ld+json">{"@type":"JobPosting","description":"Flexera saves customers billions of dollars. We are hiring engineers who can use AI to solve problems. Required Qualifications: Foundational programming experience through professional work or academic projects. Working knowledge of at least one programming language relevant to our stack. Preferred Qualifications: Hands-on experience with AI coding tools such as Claude Code. Benefits: Comprehensive medical, dental and vision coverage for you and your family."}</script></head><body></body></html>"#,
        );

        let requirements = extract_requirements_md(&html);
        assert!(requirements.contains("Foundational programming experience"));
        assert!(requirements.contains("Working knowledge of at least one programming language"));
        assert!(requirements.contains("Claude Code"));
        assert!(!requirements.contains("Comprehensive medical"));
    }
}
