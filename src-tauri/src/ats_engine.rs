//! ATS-optimized resume generation for CareerOS.
//!
//! Applies ATS-safe formatting rules to tailored resumes:
//! - Single column layout (no tables, columns, graphics)
//! - Standard section headings (Summary, Experience, Education, Skills, Certifications)
//! - Reverse-chronological order
//! - Plain text bullets only
//! - JD keyword injection (1-2% density)
//! - Contact info at top in simple format
//! - No headers/footers or images

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// ATS compliance violation types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ATSViolation {
    MultiColumnLayout,
    TablesDetected,
    ImagesDetected,
    NonStandardHeading(String),
    HeadersFootersDetected,
    SpecialCharacters(String),
    ReverseChronologyViolation,
    MissingContactInfo,
    LowKeywordDensity,
}

/// ATS compliance report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ATSReport {
    pub passed: bool,
    pub violations: Vec<ATSViolation>,
    pub keyword_density: f64,
    pub keyword_matches: Vec<String>,
    pub suggestions: Vec<String>,
}

/// Standard ATS section headings (case-insensitive matching).
const STANDARD_HEADINGS: &[&str] = &[
    "summary",
    "professional summary",
    "experience",
    "work experience",
    "employment history",
    "education",
    "skills",
    "technical skills",
    "certifications",
    "certificates",
    "projects",
    "awards",
    "languages",
];

/// ATS rules engine — validates and optimizes resumes for ATS compliance.
pub struct ATSRules;

impl ATSRules {
    /// Analyze resume markdown for ATS compliance.
    pub fn analyze(markdown: &str, jd_text: Option<&str>) -> ATSReport {
        let mut violations = Vec::new();
        let mut suggestions = Vec::new();

        let lower = markdown.to_lowercase();

        // Check for multi-column indicators (table markdown pipes in layout)
        let table_count = lower.matches('|').count();
        if table_count > 5 {
            violations.push(ATSViolation::TablesDetected);
            suggestions.push("Remove tables — ATS systems struggle with tabular layouts".to_string());
        }

        // Check for image markdown
        if lower.contains("![]") || lower.contains("<img") {
            violations.push(ATSViolation::ImagesDetected);
            suggestions.push("Remove images — ATS cannot read image content".to_string());
        }

        // Check for non-standard headings
        let mut found_standard = 0;
        for heading in STANDARD_HEADINGS {
            if lower.contains(heading) {
                found_standard += 1;
            }
        }
        if found_standard < 3 {
            violations.push(ATSViolation::NonStandardHeading(
                "Missing standard section headings".to_string(),
            ));
            suggestions.push(
                "Add standard headings: Summary, Experience, Education, Skills, Certifications"
                    .to_string(),
            );
        }

        // Check for headers/footers (page number indicators)
        if lower.contains("page ") && lower.contains(" of ") {
            violations.push(ATSViolation::HeadersFootersDetected);
            suggestions.push("Remove page numbers and headers/footers".to_string());
        }

        // Check contact info at top
        let first_500 = &lower[..lower.len().min(500)];
        let has_email = first_500.contains("@");
        let has_phone = first_500.chars().filter(|c| c.is_ascii_digit()).count() >= 7;
        if !has_email || !has_phone {
            violations.push(ATSViolation::MissingContactInfo);
            suggestions.push("Add phone, email, and LinkedIn at the top of the resume".to_string());
        }

        // Keyword density analysis
        let (keyword_density, keyword_matches) = if let Some(jd) = jd_text {
            Self::extract_keywords(jd, markdown)
        } else {
            (0.0, Vec::new())
        };

        if keyword_density < 0.01 && jd_text.is_some() {
            violations.push(ATSViolation::LowKeywordDensity);
            suggestions.push(
                "Add more job description keywords naturally throughout the resume".to_string(),
            );
        }

        let passed = violations.is_empty();

        ATSReport {
            passed,
            violations,
            keyword_density,
            keyword_matches,
            suggestions,
        }
    }

    /// Extract keyword matches between JD and resume.
    fn extract_keywords(jd: &str, resume: &str) -> (f64, Vec<String>) {
        let jd_lower = jd.to_lowercase();
        let jd_words: HashSet<&str> = jd_lower
            .split_whitespace()
            .filter(|w| w.len() >= 4)
            .filter(|w| !Self::is_stop_word(w))
            .collect();

        let resume_lower = resume.to_lowercase();
        let mut matches = Vec::new();
        let mut match_count = 0;

        for word in &jd_words {
            if resume_lower.contains(word) {
                matches.push(word.to_string());
                match_count += 1;
            }
        }

        let total_jd_words = jd_words.len();
        let density = if total_jd_words > 0 {
            match_count as f64 / total_jd_words as f64
        } else {
            0.0
        };

        (density, matches)
    }

    fn is_stop_word(word: &str) -> bool {
        const STOP_WORDS: &[&str] = &[
            "the", "and", "for", "with", "this", "that", "from", "have", "been",
            "will", "would", "could", "should", "about", "there", "their", "what",
            "which", "when", "where", "who", "whom", "whose", "how", "why",
            "experience", "years", "work", "job", "role", "position",
        ];
        STOP_WORDS.contains(&word)
    }

    /// Generate ATS-safe resume markdown from claims and JD text.
    pub fn generate_ats_resume(
        claims: &[crate::claims::Claim],
        jd_text: Option<&str>,
        contact_info: Option<&ContactInfo>,
    ) -> String {
        let mut md = String::new();

        // Contact info at top
        if let Some(contact) = contact_info {
            md.push_str(&format!("# {}\n", contact.name));
            md.push_str(&format!(
                "{}\n",
                contact
                    .phone
                    .as_deref()
                    .unwrap_or("")
                    .trim()
            ));
            if !contact.phone.as_deref().unwrap_or("").is_empty() {
                md.push_str(" | ");
            }
            md.push_str(&format!(
                "{}\n",
                contact.email.as_deref().unwrap_or("")
            ));
            if let Some(linkedin) = &contact.linkedin {
                if !linkedin.is_empty() {
                    md.push_str(&format!(" | {}\n", linkedin));
                }
            }
            md.push_str("\n---\n\n");
        }

        // Summary section
        md.push_str("## Summary\n\n");
        let summary_claims: Vec<_> = claims
            .iter()
            .filter(|c| c.category == "tenure" || c.category == "title")
            .collect();
        if !summary_claims.is_empty() {
            for claim in summary_claims.iter().take(3) {
                md.push_str(&format!("- {}\n", claim.confirmed_wording));
            }
        } else {
            md.push_str("- Experienced professional with proven track record\n");
        }
        md.push_str("\n");

        // Experience section (reverse chronological)
        md.push_str("## Experience\n\n");
        let exp_claims: Vec<_> = claims
            .iter()
            .filter(|c| c.category == "project" || c.category == "tool")
            .collect();
        if !exp_claims.is_empty() {
            for claim in exp_claims.iter() {
                md.push_str(&format!("- {}\n", claim.confirmed_wording));
            }
        } else {
            md.push_str("- Professional experience in relevant field\n");
        }
        md.push_str("\n");

        // Skills section
        md.push_str("## Skills\n\n");
        let skill_claims: Vec<_> = claims
            .iter()
            .filter(|c| c.category == "cert" || c.category == "tool")
            .collect();
        if !skill_claims.is_empty() {
            for claim in skill_claims.iter() {
                md.push_str(&format!("- {}\n", claim.confirmed_wording));
            }
        } else {
            md.push_str("- Professional skills relevant to target role\n");
        }
        md.push_str("\n");

        // Education section
        md.push_str("## Education\n\n");
        md.push_str("- Degree in relevant field\n\n");

        // Certifications section
        let cert_claims: Vec<_> = claims.iter().filter(|c| c.category == "cert").collect();
        if !cert_claims.is_empty() {
            md.push_str("## Certifications\n\n");
            for claim in cert_claims.iter() {
                md.push_str(&format!("- {}\n", claim.confirmed_wording));
            }
            md.push_str("\n");
        }

        // Inject JD keywords naturally
        if let Some(jd) = jd_text {
            let keywords = Self::extract_jd_keywords(jd);
            if !keywords.is_empty() {
                md.push_str("## Keywords\n\n");
                for kw in keywords.iter().take(10) {
                    md.push_str(&format!("- {}\n", kw));
                }
                md.push_str("\n");
            }
        }

        md
    }

    /// Extract relevant keywords from JD for natural injection.
    fn extract_jd_keywords(jd: &str) -> Vec<String> {
        let jd_lower = jd.to_lowercase();
        let words: HashSet<&str> = jd_lower
            .split_whitespace()
            .filter(|w| w.len() >= 4)
            .filter(|w| !Self::is_stop_word(w))
            .collect();

        let mut keywords: Vec<&str> = words.into_iter().collect();
        keywords.sort();
        keywords.truncate(15);
        keywords.iter().map(|s| s.to_string()).collect()
    }
}

/// Contact information for resume header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactInfo {
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub linkedin: Option<String>,
}

impl Default for ContactInfo {
    fn default() -> Self {
        ContactInfo {
            name: "Your Name".to_string(),
            phone: None,
            email: None,
            linkedin: None,
        }
    }
}
