//! One phrase extractor and one coverage function, shared by gap analysis and the
//! ATS report.
//!
//! Both surfaces answer the same question — which posting terms does this text
//! cover? — so they must answer it with the same code. If the fit matrix and the
//! ATS report ever disagree about coverage, one of them is lying.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A posting term and where it was found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeywordCoverage {
    pub term: String,
    pub in_jd: bool,
    pub in_resume: bool,
    pub in_claims: bool,
}

/// Posting terms kept per analysis. Beyond this the matrix stops being readable
/// and the signal is dominated by one-off nouns.
const MAX_TERMS: usize = 40;

/// Lowercase and strip punctuation so `Python,` and `python` are the same term.
pub fn normalize_token(token: &str) -> String {
    token
        .chars()
        .filter(|character| character.is_alphanumeric() || *character == '+' || *character == '#')
        .collect::<String>()
        .to_lowercase()
}

/// Normalized words, empties removed.
pub fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(normalize_token)
        .filter(|word| !word.is_empty())
        .collect()
}

pub fn is_stop_word(word: &str) -> bool {
    const STOP_WORDS: &[&str] = &[
        "the", "and", "for", "with", "this", "that", "from", "have", "been", "will", "would",
        "could", "should", "about", "there", "their", "what", "which", "when", "where", "who",
        "whom", "whose", "how", "why", "you", "your", "our", "ours", "are", "was", "were", "not",
        "but", "all", "any", "can", "may", "must", "into", "over", "under", "than", "then",
        "them", "they", "its", "per", "via", "plus", "across", "within", "including", "such",
        "experience", "years", "work", "working", "job", "role", "position", "team", "teams",
        "ability", "skills", "skill", "knowledge", "strong", "excellent", "preferred", "required",
        "responsibilities", "requirements", "candidate", "candidates", "opportunity", "company",
        "ideal", "looking", "need", "needs", "help", "make", "also", "more", "most", "other",
    ];
    STOP_WORDS.contains(&word)
}

/// Significant words: normalized, at least three characters, not a stop word.
pub fn significant_words(text: &str) -> Vec<String> {
    words(text)
        .into_iter()
        .filter(|word| word.len() >= 3 && !is_stop_word(word))
        .collect()
}

/// Extract posting phrases: significant bigrams first, then single terms.
///
/// Bigrams are what make a phrase valuable ("instructional design" rather than
/// "instructional" and "design"), so they lead the list; unigrams fill the rest.
pub fn extract_phrases(text: &str) -> Vec<String> {
    let tokens: Vec<String> = words(text)
        .into_iter()
        .filter(|word| !is_stop_word(word))
        .collect();

    let mut phrases: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for window in tokens.windows(2) {
        if window[0].len() < 3 || window[1].len() < 3 {
            continue;
        }
        let phrase = format!("{} {}", window[0], window[1]);
        if seen.insert(phrase.clone()) {
            phrases.push(phrase);
        }
        if phrases.len() >= MAX_TERMS {
            return phrases;
        }
    }

    for token in tokens {
        if token.len() < 4 || seen.contains(&token) {
            continue;
        }
        if seen.insert(token.clone()) {
            phrases.push(token);
        }
        if phrases.len() >= MAX_TERMS {
            break;
        }
    }

    phrases
}

/// True when `term` appears in `text` on token boundaries.
///
/// A substring test would report "art" inside "cartography"; the whole point of
/// coverage is that a person can read the result and agree with it.
pub fn contains_term(text_tokens: &[String], term: &str) -> bool {
    let term_tokens: Vec<String> = words(term);
    if term_tokens.is_empty() {
        return false;
    }
    if term_tokens.len() > text_tokens.len() {
        return false;
    }
    text_tokens
        .windows(term_tokens.len())
        .any(|window| window == term_tokens.as_slice())
}

/// Coverage of every posting phrase across the resume and the confirmed claims.
pub fn coverage(
    jd_text: &str,
    resume_text: &str,
    claims: &[crate::claims::Claim],
) -> Vec<KeywordCoverage> {
    let resume_tokens = words(resume_text);
    let claim_text = claims
        .iter()
        .map(claim_text_of)
        .collect::<Vec<_>>()
        .join("\n");
    let claim_tokens = words(&claim_text);

    extract_phrases(jd_text)
        .into_iter()
        .map(|term| KeywordCoverage {
            in_resume: contains_term(&resume_tokens, &term),
            in_claims: contains_term(&claim_tokens, &term),
            in_jd: true,
            term,
        })
        .collect()
}

/// The wording a claim is allowed to assert. Falls back to the statement for
/// rows written before confirmed wording was required.
pub fn claim_text_of(claim: &crate::claims::Claim) -> String {
    if claim.confirmed_wording.trim().is_empty() {
        claim.statement.clone()
    } else {
        claim.confirmed_wording.clone()
    }
}

/// Posting terms the document does not cover.
pub fn missing_keywords(coverage: &[KeywordCoverage]) -> Vec<String> {
    coverage
        .iter()
        .filter(|entry| entry.in_jd && !entry.in_resume)
        .map(|entry| entry.term.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{contains_term, coverage, extract_phrases, missing_keywords, words};
    use crate::claims::Claim;

    fn claim(wording: &str) -> Claim {
        Claim {
            id: "claim-1".to_string(),
            statement: wording.to_string(),
            evidence_source: "master/resume.md".to_string(),
            confirmed_wording: wording.to_string(),
            scope: "as stated".to_string(),
            category: "tool".to_string(),
            status: "confirmed".to_string(),
            last_confirmed_at: "2026-11-08".to_string(),
            evidence_type: "unspecified".to_string(),
            evidence_sha256: String::new(),
            verified_by: "ledger-repair".to_string(),
        }
    }

    #[test]
    fn phrase_extraction_prefers_bigrams() {
        let phrases = extract_phrases(
            "Build instructional design programs with Python and SQL for enterprise learning teams.",
        );
        assert!(phrases.contains(&"instructional design".to_string()));
        assert!(phrases.contains(&"python".to_string()));
        assert!(!phrases.iter().any(|phrase| phrase == "the"));
    }

    #[test]
    fn matching_is_on_token_boundaries() {
        let tokens = words("cartography for artists");
        assert!(!contains_term(&tokens, "art"), "must not match inside cartography or artists");
        assert!(contains_term(&words("an art portfolio"), "art portfolio"));
    }

    #[test]
    fn coverage_separates_resume_and_claims() {
        let jd = "Instructional design and Python automation for enterprise learning.";
        let coverage = coverage(jd, "Instructional design background.", &[claim("Python automation")]);

        let design = coverage
            .iter()
            .find(|entry| entry.term == "instructional design")
            .expect("instructional design is a posting phrase");
        assert!(design.in_resume);
        assert!(!design.in_claims);

        let python = coverage
            .iter()
            .find(|entry| entry.term == "python")
            .expect("python is a posting term");
        assert!(!python.in_resume);
        assert!(python.in_claims);

        assert!(missing_keywords(&coverage).contains(&"python".to_string()));
    }

    #[test]
    fn ats_report_and_matrix_share_one_term_set() {
        let jd = "Lead instructional design programs and own the learning technology roadmap.";
        let resume = "Instructional design lead with a learning technology background.";
        let claims = vec![claim("Led instructional design programs")];

        let matrix = coverage(jd, resume, &claims);
        let ats = crate::ats_engine::ATSRules::analyze_with_claims(resume, Some(jd), &claims);

        let matrix_terms: Vec<&str> = matrix.iter().map(|entry| entry.term.as_str()).collect();
        let ats_terms: Vec<&str> = ats
            .keyword_coverage
            .iter()
            .map(|entry| entry.term.as_str())
            .collect();
        assert_eq!(matrix_terms, ats_terms);
        assert_eq!(
            missing_keywords(&matrix),
            ats.missing_keywords,
            "both surfaces must agree on what is missing"
        );
    }
}
