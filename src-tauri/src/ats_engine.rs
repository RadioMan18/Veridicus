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

        // Check contact info at top. Truncate on a char boundary: byte slicing a
        // lowercased string can split a multi-byte character and panic.
        let first_500 = prefix_bytes(&lower, 500);
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
        // `jd_text` stays in the signature for its callers, but nothing is
        // injected from it: a keyword block is stuffing, not evidence. Matching
        // posting keywords is Stage 6b.
        let _ = jd_text;
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

        // Every section is built only from the ledger. A section with no claims is
        // omitted rather than padded with an unverifiable stock line.
        let summary_claims: Vec<_> = claims
            .iter()
            .filter(|c| c.category == "tenure" || c.category == "title")
            .collect();
        if !summary_claims.is_empty() {
            md.push_str("## Summary\n\n");
            for claim in summary_claims.iter().take(3) {
                md.push_str(&format!("- {}\n", claim.confirmed_wording));
            }
            md.push_str("\n");
        }

        // Experience section (reverse chronological)
        let exp_claims: Vec<_> = claims
            .iter()
            .filter(|c| c.category == "project" || c.category == "tool")
            .collect();
        if !exp_claims.is_empty() {
            md.push_str("## Experience\n\n");
            for claim in exp_claims.iter() {
                md.push_str(&format!("- {}\n", claim.confirmed_wording));
            }
            md.push_str("\n");
        }

        // Skills section
        let skill_claims: Vec<_> = claims
            .iter()
            .filter(|c| c.category == "cert" || c.category == "tool")
            .collect();
        if !skill_claims.is_empty() {
            md.push_str("## Skills\n\n");
            for claim in skill_claims.iter() {
                md.push_str(&format!("- {}\n", claim.confirmed_wording));
            }
            md.push_str("\n");
        }

        // Certifications section
        let cert_claims: Vec<_> = claims.iter().filter(|c| c.category == "cert").collect();
        if !cert_claims.is_empty() {
            md.push_str("## Certifications\n\n");
            for claim in cert_claims.iter() {
                md.push_str(&format!("- {}\n", claim.confirmed_wording));
            }
            md.push_str("\n");
        }

        // No education section and no keyword block: the ledger carries no degree
        // claim, and a `## Keywords` list is literal stuffing rather than content.
        md
    }
}

/// Borrow the first `max` bytes of `text` without splitting a character.
fn prefix_bytes(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
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

#[cfg(test)]
mod tests {
    use super::{ATSRules, ATSViolation};
    use crate::claims::Claim;

    fn claim(category: &str, wording: &str) -> Claim {
        Claim {
            id: "11111111-1111-4111-8111-111111111111".to_string(),
            statement: wording.to_string(),
            evidence_source: "master/resume.md".to_string(),
            confirmed_wording: wording.to_string(),
            scope: "as stated".to_string(),
            category: category.to_string(),
            status: "confirmed".to_string(),
            last_confirmed_at: "2026-11-07".to_string(),
            evidence_type: "unspecified".to_string(),
            evidence_sha256: String::new(),
            verified_by: "ledger-repair".to_string(),
        }
    }

    #[test]
    fn non_ascii_text_straddling_the_contact_boundary_does_not_panic() {
        // 300 two-byte characters push byte 500 into the middle of a character,
        // which is exactly where the old `&lower[..500]` slice panicked.
        let resume = format!(
            "Candidate Name\ncandidate@example.com\n555-123-4567\n{}",
            "é".repeat(300)
        );
        assert!(resume.len() > 500);
        let report = ATSRules::analyze(&resume, None);
        assert!(
            !report
                .violations
                .iter()
                .any(|violation| matches!(violation, ATSViolation::MissingContactInfo)),
            "contact details sit in the first bytes and must still be found"
        );
    }

    #[test]
    fn generated_resume_contains_no_fabricated_content() {
        let claims = vec![
            claim("tenure", "18+ years in enterprise healthcare IT"),
            claim("project", "Cut planning time from 8 hours to 30 minutes"),
        ];
        let output = ATSRules::generate_ats_resume(
            &claims,
            Some("learning and development python instructional design"),
            None,
        );

        assert!(output.contains("18+ years in enterprise healthcare IT"));
        assert!(output.contains("Cut planning time from 8 hours to 30 minutes"));
        assert!(!output.contains("Degree in relevant field"));
        assert!(!output.contains("Experienced professional"));
        assert!(!output.contains("Professional experience in relevant field"));
        assert!(!output.contains("## Keywords"));
    }

    #[test]
    fn empty_ledger_produces_no_padded_sections() {
        let output = ATSRules::generate_ats_resume(&[], None, None);
        assert!(output.trim().is_empty());
    }
}
