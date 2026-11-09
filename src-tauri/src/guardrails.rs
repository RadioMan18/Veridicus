//! Single enforcement point for the CareerOS honesty guardrails.
//!
//! The model proposes; these checks decide. Every generated document is evaluated
//! against the active rules, the confirmed claims ledger, the persisted gaps, and
//! the metric/scope fidelity requirements. The same evaluation runs again at commit
//! time, so a blocking finding cannot reach an approved application without an
//! explicit, recorded override.

use std::collections::HashSet;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use tauri::State;

use crate::claims::Claim;
use crate::db::get_db_connection;
use crate::fact_check;
use crate::gap_analyzer::{AnalysisResult, RequirementTier};
use crate::AppState;

pub const RESUME: &str = "resume";
pub const COVER_LETTER: &str = "cover_letter";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailFinding {
    pub code: String,
    pub message: String,
    pub blocking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GuardrailReport {
    pub findings: Vec<GuardrailFinding>,
    pub blocking: bool,
    pub passed: bool,
    pub checks_run: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleView {
    pub kind: String,
    pub subject: String,
    pub rule_text: String,
    pub applies_to: String,
}

/// A gap that must be named in the cover letter, loaded from the `gaps` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredGap {
    #[serde(default)]
    pub id: String,
    pub requirement: String,
    #[serde(default)]
    pub requirement_tier: String,
    pub adjacent_strength: String,
    #[serde(default)]
    pub status: String,
}

const SCOPE_ESCALATIONS: &[&str] = &[
    "enterprise-wide",
    "enterprise wide",
    "company-wide",
    "company wide",
    "org-wide",
    "org wide",
    "organization-wide",
    "organization wide",
    "firm-wide",
    "firm wide",
    "across the company",
    "across the organization",
];

const CONTRIBUTION_VERBS: &[&str] = &[
    "contributed",
    "contribute",
    "helped",
    "help",
    "supported",
    "support",
    "assisted",
    "assist",
    "participated",
    "participate",
];

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "have", "has", "had", "was", "were",
    "are", "will", "would", "can", "could", "should", "into", "onto", "over", "under", "about",
    "their", "there", "they", "them", "your", "our", "its", "not", "but", "all", "any", "per",
    "than", "then", "when", "where", "which", "who", "how", "why", "a", "an", "of", "to", "in",
    "on", "at", "by", "as", "is", "be", "or", "it", "we", "you", "i", "role", "job",
];

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn content_words(text: &str) -> Vec<String> {
    normalize(text)
        .split_whitespace()
        .filter(|word| word.len() >= 3 && !STOP_WORDS.contains(word))
        .map(|word| word.to_string())
        .collect()
}

fn content_word_overlap(text: &str, claim: &str) -> f32 {
    let words = content_words(text);
    if words.is_empty() {
        return 0.0;
    }
    let claim_words: HashSet<String> = content_words(claim).into_iter().collect();
    let matched = words
        .iter()
        .filter(|word| claim_words.contains(*word))
        .count();
    matched as f32 / words.len() as f32
}

fn contains_word(text: &str, word: &str) -> bool {
    text.split_whitespace().any(|token| token == word)
}

/// True when a sentence or bullet can be traced to a confirmed claim: a four-word
/// verbatim window, or content words that account for most of the bullet, or a
/// claim whose content words the bullet mostly covers.
///
/// The overlap is measured in both directions because a bullet is normally longer
/// than the claim behind it. A role line, a label or a second clause would
/// otherwise push a well-grounded bullet below the threshold, which makes the
/// check fire on honest bullets and teaches the user to ignore it.
pub fn is_grounded(text: &str, claims: &[Claim]) -> bool {
    let normalized = normalize(text);
    let words: Vec<&str> = normalized.split_whitespace().collect();
    let content = content_words(&normalized);

    for claim in claims {
        for wording in [&claim.confirmed_wording, &claim.statement] {
            let claim_normalized = normalize(wording);
            if claim_normalized.is_empty() {
                continue;
            }
            if words.len() >= 4 {
                for window in words.windows(4) {
                    if claim_normalized.contains(&window.join(" ")) {
                        return true;
                    }
                }
            }
            let claim_words: HashSet<String> =
                content_words(&claim_normalized).into_iter().collect();
            if claim_words.is_empty() || content.is_empty() {
                continue;
            }
            let matched = claim_words
                .iter()
                .filter(|word| content.iter().any(|entry| entry == *word))
                .count();
            if matched < 4 {
                continue;
            }
            let claim_covered = matched as f32 / claim_words.len() as f32;
            let bullet_covered = matched as f32 / content.len() as f32;
            if claim_covered >= 0.6 || bullet_covered >= 0.6 {
                return true;
            }
        }
    }

    false
}

/// Claims that actually appear in the document, for the audit record. This is
/// deliberately narrower than "all confirmed claims".
pub fn cited_claim_ids(draft: &str, claims: &[Claim]) -> Vec<String> {
    let units: Vec<String> = draft
        .lines()
        .flat_map(|line| line.split(|character| matches!(character, '.' | '!' | '?')))
        .map(str::trim)
        .filter(|unit| unit.len() >= 8)
        .map(str::to_string)
        .collect();

    claims
        .iter()
        .filter(|claim| {
            units
                .iter()
                .any(|unit| is_grounded(unit, std::slice::from_ref(*claim)))
        })
        .map(|claim| claim.id.clone())
        .collect()
}

/// Advisory findings for cited claims whose provenance is too thin to audit
/// meaningfully. They never block; they ask the user to strengthen the ledger.
pub fn weak_provenance_findings(claims: &[Claim], cited: &[String]) -> Vec<GuardrailFinding> {
    let mut findings = Vec::new();
    for claim in claims {
        if !cited.iter().any(|id| id == &claim.id) {
            continue;
        }
        if !crate::claims::is_weak_provenance(claim) {
            continue;
        }
        let kind = claim.evidence_type.trim().to_lowercase();
        let mut missing = Vec::new();
        if kind.is_empty() || kind == "unspecified" || kind == "other" {
            missing.push("evidence type");
        }
        if claim.evidence_source.trim().len() < 4 {
            missing.push("evidence reference");
        }
        if claim.evidence_sha256.trim().is_empty() {
            missing.push("evidence fingerprint");
        }
        if claim.verified_by.trim().is_empty() {
            missing.push("verifier");
        }
        findings.push(GuardrailFinding {
            code: "weak_claim_provenance".to_string(),
            message: format!(
                "claim '{}' has weak provenance; missing {}",
                claim.id,
                missing.join(", ")
            ),
            blocking: false,
        });
    }
    findings
}

/// Words that describe the posting's instructions or the resume's own furniture
/// rather than the missing skill itself. They would otherwise dominate the match
/// on a requirement written as a task.
const GAP_FILLER_WORDS: &[&str] = &[
    "define", "build", "work", "works", "working", "develop", "create", "partner",
    "collaborate", "lead", "support", "manage", "help", "deliver", "delivery", "ensure",
    "provide", "drive", "own", "implement", "establish", "design", "maintain", "execute",
    "coordinate", "prioritize", "contribute", "including", "include", "across", "within",
    "using", "use", "new", "existing", "concretely", "mean", "means", "role", "roles",
    "priority", "priorities", "team", "teams", "artifact", "artifacts", "education",
    "program", "programs", "ability", "experience", "strong", "excellent", "preferred",
    "required", "such", "via", "per", "each", "their", "them",
];

/// The distinctive words of a requirement, in order, with the posting's
/// instruction verbs removed.
fn gap_keywords(requirement: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    content_words(requirement)
        .into_iter()
        .filter(|word| !GAP_FILLER_WORDS.contains(&word.as_str()))
        .filter(|word| seen.insert(word.clone()))
        .take(5)
        .collect()
}

/// A requirement the posting states as a responsibility rather than a skill
/// ("Define, for each priority role, what 'AI-enabled' means ...") names nothing
/// a letter could name as a gap, so it is advisory rather than blocking.
pub fn gap_requires_naming(requirement: &str) -> bool {
    let normalized = normalize(requirement);
    let words: Vec<&str> = normalized.split_whitespace().collect();
    if words.len() < 12 {
        return true;
    }
    const TASK_OPENERS: &[&str] = &[
        "define", "build", "create", "develop", "work", "partner", "collaborate", "design",
        "implement", "manage", "lead", "support", "help", "deliver", "establish", "drive",
        "own", "ensure", "provide", "coordinate", "maintain", "execute", "translate",
    ];
    !words
        .first()
        .map(|first| TASK_OPENERS.contains(first))
        .unwrap_or(false)
}

/// True when the draft names a required gap in one sentence beside an adjacent strength.
pub fn gap_named(draft: &str, requirement: &str) -> bool {
    let draft_lower = draft.to_lowercase();
    if !requirement.trim().is_empty() && draft_lower.contains(&requirement.to_lowercase()) {
        return true;
    }

    let keywords = gap_keywords(requirement);
    if keywords.is_empty() {
        return false;
    }
    let needed = if keywords.len() >= 3 { 2 } else { keywords.len() };

    draft_lower
        .split(|character| matches!(character, '.' | '!' | '?' | '\n'))
        .any(|sentence| {
            let sentence = sentence.to_lowercase();
            let hits = keywords
                .iter()
                .filter(|keyword| sentence.contains(keyword.as_str()))
                .count();
            hits >= needed || hits * 100 / keywords.len() >= 50
        })
}

/// True when the draft mentions a name: verbatim, or by most of its distinctive
/// words. A role posted as "Senior Learning Performance Consultant, GTM" is named
/// by "... the Senior Learning Performance Consultant role".
fn mentions_value(draft: &str, value: &str) -> bool {
    let value = value.trim().to_lowercase();
    if value.is_empty() {
        return true;
    }
    if draft.to_lowercase().contains(&value) {
        return true;
    }
    let wanted = content_words(&value);
    if wanted.is_empty() {
        return false;
    }
    let draft_normalized = normalize(draft);
    let present = wanted
        .iter()
        .filter(|word| contains_word(&draft_normalized, word))
        .count();
    present * 100 / wanted.len() >= 60
}

fn best_matching_claim<'a>(sentence: &str, claims: &'a [Claim]) -> Option<&'a Claim> {
    let sentence_normalized = normalize(sentence);
    let words: Vec<&str> = sentence_normalized.split_whitespace().collect();
    let mut best: Option<(&Claim, f32)> = None;

    for claim in claims {
        let claim_normalized = normalize(&claim.confirmed_wording);
        let window_match = words
            .windows(4)
            .any(|window| claim_normalized.contains(&window.join(" ")));
        let overlap = content_word_overlap(&sentence_normalized, &claim_normalized);
        if !window_match && overlap < 0.4 {
            continue;
        }
        let score = if window_match { 1.0 } else { overlap };
        if best.map(|(_, best_score)| score > best_score).unwrap_or(true) {
            best = Some((claim, score));
        }
    }

    best.map(|(claim, _)| claim)
}

fn push(report: &mut GuardrailReport, code: &str, message: String, blocking: bool) {
    report.findings.push(GuardrailFinding {
        code: code.to_string(),
        message,
        blocking,
    });
}

fn finalize(mut report: GuardrailReport) -> GuardrailReport {
    report.blocking = report.findings.iter().any(|finding| finding.blocking);
    report.passed = !report.blocking;
    report
}

/// Section headings whose bullets are credentials or keyword lists rather than
/// statements of experience. Matched as substrings because the resume writes
/// combined headings such as "Certifications & Key Achievements".
const LIST_SECTION_HINTS: &[&str] = &[
    "education",
    "certif",
    "credential",
    "license",
    "licensure",
    "award",
    "achievement",
    "honor",
    "publication",
    "language",
    "skill",
    "competen",
    "expertise",
    "affiliation",
];

fn is_list_section(section: &str) -> bool {
    LIST_SECTION_HINTS
        .iter()
        .any(|hint| section.contains(*hint))
}

/// Verbs that make a bullet a statement of work rather than a theme heading.
const ACTION_VERBS: &[&str] = &[
    "led", "lead", "built", "build", "created", "designed", "developed", "delivered",
    "taught", "owned", "drove", "increased", "reduced", "cut", "grew", "managed",
    "implemented", "trained", "launched", "produced", "wrote", "presented", "helped",
    "partnered", "achieved", "improved", "saved", "scaled", "coached", "mentored",
    "established", "founded", "streamlined", "migrated", "automated", "supported", "ran",
    "run", "operated", "maintained", "reorganized", "analyzed", "authored", "facilitated",
    "onboarded", "negotiated", "secured", "raised", "earned", "served", "serve", "created",
];

/// A short noun phrase with no verb states no fact of its own; the resume uses
/// these as group headings ("Cross-functional leadership experience").
fn is_theme_heading(text: &str) -> bool {
    if text.contains(':') || text.chars().any(|character| character.is_ascii_digit()) {
        return false;
    }
    let normalized = normalize(text);
    let words: Vec<&str> = normalized.split_whitespace().collect();
    if words.is_empty() || words.len() > 5 {
        return false;
    }
    !words
        .iter()
        .any(|word| ACTION_VERBS.contains(word))
}

/// The part of a bullet that makes a factual statement: the text after a short
/// leading label. A label with nothing after it states no fact of its own.
fn bullet_statement(bullet: &str) -> Option<&str> {
    let Some((label, rest)) = bullet.split_once(':') else {
        return Some(bullet);
    };
    let label = label.trim();
    let rest = rest.trim();
    if label.is_empty() || label.len() > 60 {
        return Some(bullet);
    }
    if rest.is_empty() {
        return None;
    }
    if rest.len() >= 20 {
        Some(rest)
    } else {
        Some(bullet)
    }
}

/// Credentials, badges and community entries name a thing the user holds or did;
/// they are not statements of work, so they are judged by the list-section rule
/// rather than as experience claims.
const CREDENTIAL_HINTS: &[&str] = &[
    "certification",
    "certifications",
    "certificate",
    "certified",
    "credential",
    "badge",
    "award",
    "honor",
    "license",
    "licensure",
    "scholarship",
    "affiliation",
    "publication",
    "community",
];

fn strip_emphasis(text: &str) -> String {
    text.trim()
        .trim_matches(|character| matches!(character, '*' | '_'))
        .trim()
        .to_string()
}

fn has_action_verb(text: &str) -> bool {
    normalize(text)
        .split_whitespace()
        .any(|word| ACTION_VERBS.contains(&word))
}

/// A line that is resume furniture rather than a fact about work: a heading set
/// in bold with nothing after it, a theme phrase, or a credential with no verb.
fn is_structure_line(bullet: &str) -> bool {
    let trimmed = bullet.trim();
    let plain = strip_emphasis(trimmed);
    if plain.is_empty() {
        return true;
    }
    // A bolded sentence still states something, so it is judged like any bullet;
    // only a bold noun phrase is a heading.
    if has_action_verb(&plain) {
        return false;
    }
    if is_theme_heading(&plain) {
        return true;
    }
    let lower = plain.to_lowercase();
    if CREDENTIAL_HINTS.iter().any(|hint| lower.contains(hint)) {
        return true;
    }
    trimmed.starts_with("**")
        && trimmed.ends_with("**")
        && trimmed.matches("**").count() == 2
        && plain.split_whitespace().count() <= 12
        && !plain.ends_with('.')
}

/// The first line that opens with a greeting, wherever the letterhead puts it.
fn leading_salutation(draft: &str) -> Option<&str> {
    draft
        .lines()
        .map(str::trim)
        .find(|line| line.len() <= 80 && line.to_lowercase().starts_with("dear "))
}

/// Words that name the recipient's function rather than a company, so "Dear
/// Hiring Manager," is not read as a company called Hiring Manager.
const SALUTATION_GENERIC_WORDS: &[&str] = &[
    "hiring",
    "team",
    "teams",
    "manager",
    "managers",
    "recruiter",
    "recruiters",
    "recruiting",
    "talent",
    "acquisition",
    "department",
    "human",
    "resources",
    "hr",
    "sir",
    "madam",
    "folks",
    "everyone",
    "the",
    "and",
    "of",
    "at",
];

/// True when the greeting names a company other than the employer.
fn salutation_names_other_company(salutation: &str, employer: &str) -> bool {
    let salutation_lower = salutation.to_lowercase();
    let employer = employer.trim().to_lowercase();
    if employer.is_empty() || salutation_lower.contains(&employer) {
        return false;
    }
    let named: Vec<&str> = salutation_lower
        .trim_start_matches("dear")
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| word.len() > 2)
        .filter(|word| !SALUTATION_GENERIC_WORDS.contains(word))
        .collect();
    if named.is_empty() {
        return false;
    }
    // "Dear Acme team," still names Acme when the posting calls it "Acme Inc".
    if employer
        .split_whitespace()
        .any(|word| word.len() > 2 && named.contains(&word))
    {
        return false;
    }
    true
}

fn ungrounded_resume_bullets(draft: &str, claims: &[Claim]) -> Vec<String> {
    let mut findings = Vec::new();
    let mut section = String::new();

    for line in draft.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("## ") {
            section = heading.trim().to_lowercase();
            continue;
        }
        if trimmed.starts_with("###") || !trimmed.starts_with("- ") {
            continue;
        }
        // Education, certification and skills lines are credentials and keyword
        // lists, not experience claims. Numbers in them are still covered by the
        // document-wide metric check.
        if is_list_section(&section) {
            continue;
        }
        let bullet = trimmed.trim_start_matches("- ").trim();
        // A bold-only heading, a theme phrase or a credential states no fact of
        // its own, so it is not an ungrounded claim about work.
        if is_structure_line(bullet) {
            continue;
        }
        // A bullet may open with its own label ("Sunrise EHR Documentation:
        // Maintained ..."); the label is a heading, so judge the statement after it.
        let Some(statement) = bullet_statement(bullet) else {
            continue;
        };
        if statement.len() < 20 {
            continue;
        }
        // A short noun phrase is a group heading, not a claim about work.
        if is_theme_heading(statement) {
            continue;
        }
        if !is_grounded(statement, claims) {
            findings.push(format!(
                "resume bullet is not supported by a confirmed claim: '{}'",
                bullet
            ));
        }
    }

    findings
}

fn scope_findings(draft: &str, claims: &[Claim]) -> Vec<String> {
    let mut findings = Vec::new();

    for sentence in draft.split(|character| matches!(character, '.' | '!' | '?' | '\n')) {
        let sentence = sentence.trim();
        if sentence.is_empty() {
            continue;
        }
        let Some(claim) = best_matching_claim(sentence, claims) else {
            continue;
        };
        let sentence_normalized = normalize(sentence);
        let claim_normalized = normalize(&claim.confirmed_wording);

        for term in SCOPE_ESCALATIONS {
            if sentence_normalized.contains(term) && !claim_normalized.contains(term) {
                findings.push(format!(
                    "'{}' escalates the confirmed scope of '{}'",
                    sentence, claim.confirmed_wording
                ));
            }
        }

        let draft_claims_leadership = contains_word(&sentence_normalized, "led")
            || contains_word(&sentence_normalized, "lead")
            || contains_word(&sentence_normalized, "owned");
        let claim_is_contributory = CONTRIBUTION_VERBS
            .iter()
            .any(|verb| contains_word(&claim_normalized, verb));
        let claim_claims_leadership = contains_word(&claim_normalized, "led")
            || contains_word(&claim_normalized, "lead")
            || contains_word(&claim_normalized, "owned");

        if draft_claims_leadership && claim_is_contributory && !claim_claims_leadership {
            findings.push(format!(
                "'{}' claims ownership the confirmed claim does not: '{}'",
                sentence, claim.confirmed_wording
            ));
        }
    }

    findings
}

/// Evaluate one document against the active rules and the confirmed record.
pub fn evaluate_document(
    kind: &str,
    draft: &str,
    claims: &[Claim],
    rules: &[RuleView],
    employer: Option<&str>,
    role: Option<&str>,
    required_gaps: &[RequiredGap],
) -> GuardrailReport {
    let mut report = GuardrailReport::default();
    let draft_lower = draft.to_lowercase();

    for rule in rules {
        let subject = rule.subject.trim();
        if subject.is_empty() {
            continue;
        }
        report.checks_run += 1;
        let subject_lower = subject.to_lowercase();

        match rule.kind.as_str() {
            "never_claim" => {
                if draft_lower.contains(&subject_lower) {
                    push(
                        &mut report,
                        "never_claim",
                        format!("the draft mentions a never-claim subject: '{}'", subject),
                        true,
                    );
                }
            }
            "ask_first" => {
                if draft_lower.contains(&subject_lower) {
                    push(
                        &mut report,
                        "ask_first",
                        format!(
                            "the draft uses '{}', which must be confirmed by the user first",
                            subject
                        ),
                        true,
                    );
                }
            }
            "exact_wording" => {
                if draft_lower.contains(&subject_lower) {
                    let approved = rule.rule_text.trim();
                    if !approved.is_empty() && !draft_lower.contains(&approved.to_lowercase()) {
                        push(
                            &mut report,
                            "exact_wording",
                            format!(
                                "the approved wording for '{}' was altered: expected '{}'",
                                subject, approved
                            ),
                            true,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    let metrics = fact_check::verify_metrics(draft, claims);
    report.checks_run += metrics.found.max(1);
    for metric in &metrics.unverified {
        push(
            &mut report,
            "unverified_metric",
            format!("'{}' is not supported by any confirmed claim", metric),
            true,
        );
    }
    for violation in &metrics.qualifier_violations {
        push(&mut report, "qualifier_dropped", violation.clone(), true);
    }

    for message in scope_findings(draft, claims) {
        report.checks_run += 1;
        push(&mut report, "scope_escalation", message, true);
    }

    if kind == COVER_LETTER {
        for gap in required_gaps {
            if gap.status == "accepted_by_user" {
                continue;
            }
            report.checks_run += 1;
            if !gap_named(draft, &gap.requirement) {
                // A responsibility written as a task names no missing skill, so it
                // stays visible without stopping an otherwise honest letter.
                let blocking = gap_requires_naming(&gap.requirement);
                push(
                    &mut report,
                    "gap_not_named",
                    format!("the letter must name this gap: {}", gap.requirement),
                    blocking,
                );
            }
        }

        for (label, value) in [("employer", employer), ("role", role)] {
            let Some(value) = value else {
                continue;
            };
            let value = value.trim();
            if value.is_empty() || value.eq_ignore_ascii_case("unknown") {
                continue;
            }
            report.checks_run += 1;
            if !mentions_value(draft, value) {
                push(
                    &mut report,
                    "role_company_mismatch",
                    format!("the letter does not name the {}", label),
                    true,
                );
            }
        }

        // A greeting that names somebody else's company is the one place a wrong
        // employer is not caught by the checks above: the header can carry the
        // right name while the salutation still says "Dear Aurora".
        if let (Some(employer), Some(salutation)) = (employer, leading_salutation(draft)) {
            report.checks_run += 1;
            if salutation_names_other_company(salutation, employer) {
                push(
                    &mut report,
                    "role_company_mismatch",
                    format!(
                        "the letter greets a different employer: '{}'",
                        salutation.trim()
                    ),
                    true,
                );
            }
        }
    }

    if kind == RESUME {
        for message in ungrounded_resume_bullets(draft, claims) {
            report.checks_run += 1;
            push(&mut report, "ungrounded_bullet", message, true);
        }
    }

    finalize(report)
}

pub fn merge(reports: Vec<GuardrailReport>) -> GuardrailReport {
    let mut merged = GuardrailReport::default();
    for report in reports {
        merged.findings.extend(report.findings);
        merged.checks_run += report.checks_run;
    }
    merged.blocking = merged.findings.iter().any(|finding| finding.blocking);
    merged.passed = !merged.blocking;
    merged
}

pub fn load_active_rules(conn: &Connection, kind: &str) -> Result<Vec<RuleView>, String> {
    let mut statement = conn
        .prepare("SELECT kind, subject, rule_text, applies_to FROM rules WHERE active = 1 AND applies_to IN ('all', ?1)")
        .map_err(|error| format!("Failed to prepare rule query: {}", error))?;

    let rules = statement
        .query_map([kind], |row| {
            Ok(RuleView {
                kind: row.get(0)?,
                subject: row.get(1)?,
                rule_text: row.get(2)?,
                applies_to: row.get(3)?,
            })
        })
        .map_err(|error| format!("Failed to query rules: {}", error))?
        .filter_map(|rule| rule.ok())
        .collect();

    Ok(rules)
}

pub fn fetch_confirmed_claims(conn: &Connection) -> Result<Vec<Claim>, String> {
    let mut statement = conn
        .prepare("SELECT id, statement, evidence_source, confirmed_wording, scope, category, status, last_confirmed_at, evidence_type, evidence_sha256, verified_by FROM claims WHERE status = 'confirmed'")
        .map_err(|error| format!("Failed to prepare claims query: {}", error))?;

    let claims = statement
        .query_map([], |row| {
            Ok(Claim {
                id: row.get(0)?,
                statement: row.get(1)?,
                evidence_source: row.get(2)?,
                confirmed_wording: row.get(3)?,
                scope: row.get(4)?,
                category: row.get(5)?,
                status: row.get(6)?,
                last_confirmed_at: row.get(7)?,
                evidence_type: row.get(8)?,
                evidence_sha256: row.get(9)?,
                verified_by: row.get(10)?,
            })
        })
        .map_err(|error| format!("Failed to query claims: {}", error))?
        .filter_map(|claim| claim.ok())
        .collect();

    Ok(claims)
}

fn tier_name(tier: &RequirementTier) -> &'static str {
    match tier {
        RequirementTier::MustHave => "must_have",
        RequirementTier::NiceToHave => "nice_to_have",
        RequirementTier::Gate => "gate",
    }
}

/// Persist the analysed gaps, keyed on (application, requirement).
///
/// Re-analysis refreshes the tier and the matrix text but keeps the row id, the
/// user's decision and any attached stories. The old delete-and-insert wiped a
/// choice the moment `/apply` ran again.
pub fn persist_gaps(
    conn: &Connection,
    application_id: &str,
    analysis: &AnalysisResult,
) -> Result<Vec<RequiredGap>, String> {
    let mut saved = Vec::new();
    for gap in &analysis.identified_gaps {
        let tier = tier_name(&gap.requirement_tier);
        // A gap by definition has no matching claim, so this must bind NULL.
        // An empty string is not NULL and violates the `REFERENCES claims(id)`
        // foreign key, which aborted every /apply that found a gap.
        let existing = match conn.query_row(
            "SELECT id, status FROM gaps WHERE application_id = ?1 AND requirement = ?2",
            rusqlite::params![application_id, gap.requirement],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ) {
            Ok(row) => Some(row),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(error) => return Err(format!("Failed to look up existing gap: {}", error)),
        };

        let (id, status) = match existing {
            Some((id, status)) => {
                conn.execute(
                    "UPDATE gaps SET requirement_tier = ?1, matched_claim_id = ?2, adjacent_strength = ?3, letter_line = ?4, cv_focus = ?5, cover_letter_point = ?6 WHERE id = ?7",
                    rusqlite::params![
                        tier,
                        gap.matched_claim_id,
                        gap.adjacent_strength,
                        gap.proposed_letter_line,
                        gap.cv_focus,
                        gap.cover_letter_point,
                        id,
                    ],
                )
                .map_err(|error| format!("Failed to update gap: {}", error))?;
                (id, status)
            }
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO gaps (id, application_id, requirement, requirement_tier, matched_claim_id, adjacent_strength, letter_line, cv_focus, cover_letter_point, story_ids, status)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '', 'open')",
                    rusqlite::params![
                        id,
                        application_id,
                        gap.requirement,
                        tier,
                        gap.matched_claim_id,
                        gap.adjacent_strength,
                        gap.proposed_letter_line,
                        gap.cv_focus,
                        gap.cover_letter_point,
                    ],
                )
                .map_err(|error| format!("Failed to persist gap: {}", error))?;
                (id, "open".to_string())
            }
        };

        saved.push(RequiredGap {
            id,
            requirement: gap.requirement.clone(),
            requirement_tier: tier.to_string(),
            adjacent_strength: gap.adjacent_strength.clone(),
            status,
        });
    }

    Ok(saved)
}

/// Requirements the user chose to leave unaddressed.
///
/// They stop being required in the letter, but the gate still reports them as a
/// deliberate omission instead of letting the application pass in silence.
pub fn accepted_gap_requirements(conn: &Connection, application_id: &str) -> Vec<String> {
    let Ok(mut statement) = conn.prepare(
        "SELECT requirement FROM gaps WHERE application_id = ?1 AND status = 'accepted_by_user'",
    ) else {
        return Vec::new();
    };

    statement
        .query_map([application_id], |row| row.get::<_, String>(0))
        .map(|rows| rows.filter_map(|row| row.ok()).collect())
        .unwrap_or_default()
}

/// One persisted gap, including the fields the fit matrix renders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapRow {
    pub id: String,
    pub requirement: String,
    pub requirement_tier: String,
    pub adjacent_strength: String,
    pub letter_line: Option<String>,
    pub cv_focus: String,
    pub cover_letter_point: String,
    pub story_ids: Vec<String>,
    pub status: String,
}

fn open_connection(state: &State<'_, AppState>) -> Result<Connection, String> {
    let db_path = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt
            .as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    get_db_connection(&db_path)
}

/// Gaps for one application, with the matrix fields.
#[tauri::command]
pub fn list_gaps(
    application_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<GapRow>, String> {
    let conn = open_connection(&state)?;
    let mut statement = conn
        .prepare(
            "SELECT id, requirement, requirement_tier, adjacent_strength, letter_line, cv_focus, cover_letter_point, story_ids, status
             FROM gaps WHERE application_id = ?1 ORDER BY requirement_tier, requirement",
        )
        .map_err(|error| format!("Failed to prepare gap query: {}", error))?;

    let rows = statement
        .query_map([application_id], |row| {
            let story_ids: String = row.get(7)?;
            Ok(GapRow {
                id: row.get(0)?,
                requirement: row.get(1)?,
                requirement_tier: row.get(2)?,
                adjacent_strength: row.get(3)?,
                letter_line: row.get(4)?,
                cv_focus: row.get(5)?,
                cover_letter_point: row.get(6)?,
                story_ids: story_ids
                    .split(',')
                    .map(|id| id.trim().to_string())
                    .filter(|id| !id.is_empty())
                    .collect(),
                status: row.get(8)?,
            })
        })
        .map_err(|error| format!("Failed to query gaps: {}", error))?
        .filter_map(|row| row.ok())
        .collect();

    Ok(rows)
}

/// Record a gap decision. Must-haves remain enforced unless the user explicitly
/// accepts the gap as unaddressed, which the gate then reports as a finding.
#[tauri::command]
pub fn set_gap_status(
    gap_id: String,
    status: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    const ALLOWED: [&str; 3] = ["open", "named_in_letter", "accepted_by_user"];
    if !ALLOWED.contains(&status.as_str()) {
        return Err(format!("Unknown gap status: {}", status));
    }

    let conn = open_connection(&state)?;
    let updated = conn
        .execute(
            "UPDATE gaps SET status = ?1 WHERE id = ?2",
            rusqlite::params![status, gap_id],
        )
        .map_err(|error| format!("Failed to update the gap: {}", error))?;

    if updated == 0 {
        return Err("No gap with that id".to_string());
    }

    Ok(true)
}

/// Gaps that must be named in the letter: must-have and gate items still open.
pub fn required_gaps_for(
    conn: &Connection,
    application_id: &str,
) -> Result<Vec<RequiredGap>, String> {
    let mut statement = conn
        .prepare(
            "SELECT id, requirement, requirement_tier, adjacent_strength, status
             FROM gaps
             WHERE application_id = ?1
               AND status != 'accepted_by_user'
               AND requirement_tier IN ('must_have', 'gate')",
        )
        .map_err(|error| format!("Failed to prepare gap query: {}", error))?;

    let gaps = statement
        .query_map([application_id], |row| {
            Ok(RequiredGap {
                id: row.get(0)?,
                requirement: row.get(1)?,
                requirement_tier: row.get(2)?,
                adjacent_strength: row.get(3)?,
                status: row.get(4)?,
            })
        })
        .map_err(|error| format!("Failed to query gaps: {}", error))?
        .filter_map(|gap| gap.ok())
        .collect();

    Ok(gaps)
}

#[cfg(test)]
mod gap_decision_tests {
    use super::{accepted_gap_requirements, persist_gaps};
    use crate::gap_analyzer::{AnalysisResult, GapEntry, RequirementTier};

    fn analysis() -> AnalysisResult {
        AnalysisResult {
            supported_claim_ids: Vec::new(),
            identified_gaps: vec![GapEntry {
                requirement: "Own the learning technology roadmap".to_string(),
                requirement_tier: RequirementTier::MustHave,
                matched_claim_id: None,
                adjacent_strength: "adjacent".to_string(),
                proposed_letter_line: "line".to_string(),
                cv_focus: "focus".to_string(),
                cover_letter_point: "point".to_string(),
                story_ids: Vec::new(),
            }],
            ask_first_triggered: Vec::new(),
            ambiguous_requirements: Vec::new(),
            hiring_goals: Vec::new(),
            standout_traits: Vec::new(),
            ats_keywords: Vec::new(),
        }
    }

    fn database() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE application_records (
                id TEXT PRIMARY KEY, employer TEXT NOT NULL, role TEXT NOT NULL, req_id TEXT,
                date TEXT NOT NULL, base_resume_version TEXT NOT NULL, materials_version TEXT NOT NULL,
                claims_used TEXT NOT NULL, approved_wording TEXT NOT NULL, gaps_named TEXT NOT NULL,
                salary_entered TEXT, status TEXT, notes TEXT);
             CREATE TABLE claims (id TEXT PRIMARY KEY);
             CREATE TABLE gaps (
                id TEXT PRIMARY KEY, application_id TEXT NOT NULL, requirement TEXT NOT NULL,
                requirement_tier TEXT, matched_claim_id TEXT REFERENCES claims(id),
                adjacent_strength TEXT NOT NULL, letter_line TEXT, status TEXT,
                FOREIGN KEY(application_id) REFERENCES application_records(id));",
        )
        .expect("schema");
        crate::db::ensure_gap_columns(&conn).expect("migration");
        conn.execute(
            "INSERT INTO application_records (id, employer, role, date, base_resume_version, materials_version, claims_used, approved_wording, gaps_named)
             VALUES ('app-1','Acme','Engineer','2026-11-08','v1','v1','','','0')",
            [],
        )
        .expect("application");
        conn
    }

    #[test]
    fn a_user_gap_decision_survives_reanalysis() {
        let conn = database();
        let first = persist_gaps(&conn, "app-1", &analysis()).expect("first persist");
        assert_eq!(first.len(), 1);
        conn.execute(
            "UPDATE gaps SET status = 'accepted_by_user' WHERE id = ?1",
            [&first[0].id],
        )
        .expect("decision");

        let second = persist_gaps(&conn, "app-1", &analysis()).expect("second persist");
        assert_eq!(second[0].id, first[0].id, "the gap keeps its identity");
        assert_eq!(second[0].status, "accepted_by_user", "the decision survives");

        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM gaps", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 1, "re-analysis must not duplicate rows");
    }

    #[test]
    fn accepted_gaps_are_reported_not_silently_skipped() {
        let conn = database();
        let saved = persist_gaps(&conn, "app-1", &analysis()).expect("persist");
        conn.execute(
            "UPDATE gaps SET status = 'accepted_by_user' WHERE id = ?1",
            [&saved[0].id],
        )
        .unwrap();

        let accepted = accepted_gap_requirements(&conn, "app-1");
        assert_eq!(accepted.len(), 1);
        assert!(accepted[0].contains("learning technology"));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        evaluate_document, gap_named, is_grounded, merge, RequiredGap, RuleView, COVER_LETTER,
        RESUME,
    };
    use crate::claims::Claim;

    fn claim(wording: &str) -> Claim {
        Claim {
            id: "claim-1".to_string(),
            statement: wording.to_string(),
            evidence_source: "resume".to_string(),
            confirmed_wording: wording.to_string(),
            scope: "within the team".to_string(),
            category: "project".to_string(),
            status: "confirmed".to_string(),
            last_confirmed_at: "2026-01-01".to_string(),
            evidence_type: "performance_review".to_string(),
            evidence_sha256: "abc".to_string(),
            verified_by: "Jane".to_string(),
        }
    }

    fn rule(kind: &str, subject: &str, rule_text: &str) -> RuleView {
        RuleView {
            kind: kind.to_string(),
            subject: subject.to_string(),
            rule_text: rule_text.to_string(),
            applies_to: "all".to_string(),
        }
    }

    fn gap(requirement: &str) -> RequiredGap {
        RequiredGap {
            id: "gap-1".to_string(),
            requirement: requirement.to_string(),
            requirement_tier: "must_have".to_string(),
            adjacent_strength: "adjacent work".to_string(),
            status: "open".to_string(),
        }
    }

    #[test]
    fn never_claim_subject_blocks_the_document() {
        let report = evaluate_document(
            COVER_LETTER,
            "Dear team, I hold a security clearance and delivered work.",
            &[claim("Delivered work for the team")],
            &[rule("never_claim", "security clearance", "")],
            Some("Acme"),
            Some("Engineer"),
            &[],
        );

        assert!(report.blocking);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "never_claim"));
    }

    #[test]
    fn ask_first_subject_blocks_until_confirmed() {
        let report = evaluate_document(
            RESUME,
            "- Delivered work for the team using Kubernetes clusters.",
            &[claim("Delivered work for the team")],
            &[rule("ask_first", "kubernetes", "")],
            None,
            None,
            &[],
        );

        assert!(report.blocking);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "ask_first"));
    }

    #[test]
    fn altered_exact_wording_blocks() {
        let report = evaluate_document(
            COVER_LETTER,
            "We cut planning time from eight hours to thirty minutes. Acme needs an Engineer.",
            &[claim("Reduced planning time from 8 hours to 30 minutes")],
            &[rule(
                "exact_wording",
                "planning time",
                "from 8 hours to 30 minutes",
            )],
            Some("Acme"),
            Some("Engineer"),
            &[],
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "exact_wording"));
    }

    #[test]
    fn dropped_metric_qualifier_blocks() {
        let report = evaluate_document(
            COVER_LETTER,
            "That change reached 50% adoption across the team. Acme needs an Engineer.",
            &[claim("Reached up to 50% adoption across the team")],
            &[],
            Some("Acme"),
            Some("Engineer"),
            &[],
        );

        assert!(report.blocking);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "qualifier_dropped"));
    }

    #[test]
    fn confirmed_qualifier_passes() {
        let report = evaluate_document(
            COVER_LETTER,
            "That change reached up to 50% adoption across the team. Acme needs an Engineer.",
            &[claim("Reached up to 50% adoption across the team")],
            &[],
            Some("Acme"),
            Some("Engineer"),
            &[],
        );

        assert!(!report.blocking, "findings: {:?}", report.findings);
    }

    #[test]
    fn scope_escalation_blocks() {
        let report = evaluate_document(
            COVER_LETTER,
            "I built 10 internal tools for the team and led enterprise-wide adoption. Acme needs an Engineer.",
            &[claim("Built 10 internal tools for the team")],
            &[],
            Some("Acme"),
            Some("Engineer"),
            &[],
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "scope_escalation"));
    }

    #[test]
    fn required_gap_must_be_named() {
        let report = evaluate_document(
            COVER_LETTER,
            "I have relevant experience. Acme needs an Engineer.",
            &[claim("Delivered relevant experience")],
            &[],
            Some("Acme"),
            Some("Engineer"),
            &[gap("Kubernetes cluster administration")],
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "gap_not_named"));
    }

    #[test]
    fn named_gap_passes() {
        let report = evaluate_document(
            COVER_LETTER,
            "I have not administered Kubernetes clusters by name, but I ran comparable container infrastructure. Acme needs an Engineer.",
            &[claim("Ran comparable container infrastructure")],
            &[],
            Some("Acme"),
            Some("Engineer"),
            &[gap("Kubernetes cluster administration")],
        );

        assert!(!report.blocking, "findings: {:?}", report.findings);
    }

    #[test]
    fn a_role_named_without_its_trailing_qualifier_is_still_named() {
        let report = evaluate_document(
            COVER_LETTER,
            "Dear Acme Hiring Team,\n\nI am applying for the Senior Learning Performance Consultant role.",
            &[claim("Applying for the role")],
            &[],
            Some("Acme"),
            Some("Senior Learning Performance Consultant, GTM"),
            &[],
        );

        assert!(
            !report
                .findings
                .iter()
                .any(|finding| finding.code == "role_company_mismatch"),
            "findings: {:?}",
            report.findings
        );
    }

    #[test]
    fn a_requirement_written_as_a_task_is_advisory() {
        let report = evaluate_document(
            COVER_LETTER,
            "Acme needs an Engineer. I built AI tooling for training teams.",
            &[claim("Built AI tooling for training teams")],
            &[],
            Some("Acme"),
            Some("Engineer"),
            &[gap(
                "Define, for each priority role, what 'AI-enabled' concretely means in the workflow, and build the artifacts that deliver it",
            )],
        );

        assert!(!report.blocking, "findings: {:?}", report.findings);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "gap_not_named" && !finding.blocking));
    }

    #[test]
    fn certification_and_achievement_bullets_are_not_experience_claims() {
        let draft = "## Certifications & Key Achievements\n- Six Sigma Green Belt Certification (08/2007)\n- Altera Company-Wide Innovation Badge\n- AI Community Building\n";
        let report = evaluate_document(
            RESUME,
            draft,
            &[claim("Built a telemetry pipeline used by twelve teams")],
            &[],
            None,
            None,
            &[],
        );

        assert!(report.findings.is_empty(), "findings: {:?}", report.findings);
    }

    #[test]
    fn group_headings_are_not_treated_as_experience_bullets() {
        let draft = "## Additional Leadership\n- Cross-functional leadership experience\n- Leadership impact outside dated roles\n- Led a cross-country team of 40 trainers through a merger\n";
        let report = evaluate_document(
            RESUME,
            draft,
            &[claim("Led a cross-country team of 40 trainers through a merger")],
            &[],
            None,
            None,
            &[],
        );

        assert!(report.findings.is_empty(), "findings: {:?}", report.findings);
    }

    #[test]
    fn invented_resume_bullet_blocks_but_education_is_skipped() {
        let draft = "# Jane\n\n## Experience\n- Built a telemetry pipeline used by twelve teams\n- Invented a company-wide platform nobody confirmed\n\n## Education\n- B.S. Computer Science, 2014";
        let report = evaluate_document(
            RESUME,
            draft,
            &[claim("Built a telemetry pipeline used by twelve teams")],
            &[],
            None,
            None,
            &[],
        );

        let ungrounded: Vec<_> = report
            .findings
            .iter()
            .filter(|finding| finding.code == "ungrounded_bullet")
            .collect();
        assert_eq!(ungrounded.len(), 1);
        assert!(ungrounded[0].message.contains("Invented a company-wide platform"));
    }

    #[test]
    fn an_expanded_bullet_resting_on_a_claim_is_grounded() {
        let draft = "## Experience\n- Supported Altera TouchWorks go-lives across the U.S., including end-user readiness and post-live optimization for clinical staff.\n";
        let report = evaluate_document(
            RESUME,
            draft,
            &[claim("Supported 50+ enterprise go-lives for Altera TouchWorks EHR.")],
            &[],
            None,
            None,
            &[],
        );

        assert!(report.findings.is_empty(), "findings: {:?}", report.findings);
    }

    #[test]
    fn a_labelled_bullet_is_judged_on_the_statement_after_the_label() {
        let draft = "## Experience\n- Sunrise EHR Documentation: Reorganized the ILT Courseware SharePoint structure, cutting asset search time from 30 minutes to under 5 minutes.\n";
        let report = evaluate_document(
            RESUME,
            draft,
            &[claim("Reorganized the ILT Courseware SharePoint structure, cutting asset search time from 30 minutes to under 5 minutes")],
            &[],
            None,
            None,
            &[],
        );

        assert!(report.findings.is_empty(), "findings: {:?}", report.findings);
    }

    #[test]
    fn resume_headings_and_credentials_are_not_bullets() {
        let draft = "## Additional Leadership\n- **Cross-functional leadership experience**\n- **Cross-country leadership experience**\n- **Leadership impact outside dated roles**\n- **Six Sigma Green Belt Certification (08/2007)**\n- **Altera Company-Wide Innovation Badge**\n- **AI Community Building**\n";
        let report = evaluate_document(
            RESUME,
            draft,
            &[claim("Led the training program")],
            &[],
            None,
            None,
            &[],
        );

        let ungrounded: Vec<_> = report
            .findings
            .iter()
            .filter(|finding| finding.code == "ungrounded_bullet")
            .collect();
        assert!(ungrounded.is_empty(), "findings: {:?}", ungrounded);
    }

    #[test]
    fn a_bolded_sentence_is_still_checked() {
        let draft =
            "## Experience\n- **Built a company-wide platform nobody confirmed for the whole division**\n";
        let report = evaluate_document(
            RESUME,
            draft,
            &[claim("Led the training program")],
            &[],
            None,
            None,
            &[],
        );

        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "ungrounded_bullet"),
            "findings: {:?}",
            report.findings
        );
    }

    #[test]
    fn a_letter_greeting_another_company_blocks() {
        let draft = "**DAVID W. BOUKNIGHT**\n\n**Senior Learning Performance Consultant, GTM**\n\n**iCIMS**\n\nDear Aurora Hiring Team,\n\nI cut reporting costs by 12%.";
        let report = evaluate_document(
            COVER_LETTER,
            draft,
            &[claim("Cut reporting costs by 12%")],
            &[],
            Some("iCIMS"),
            Some("Senior Learning Performance Consultant, GTM"),
            &[],
        );

        assert!(
            report.findings.iter().any(|finding| finding.code
                == "role_company_mismatch"
                && finding.message.contains("Aurora")),
            "findings: {:?}",
            report.findings
        );
    }

    #[test]
    fn a_task_style_requirement_is_advisory_not_blocking() {
        let requirement = "Define, for each priority role, what 'AI-enabled' concretely means in the workflow — which decisions, which reports, which repetitive steps — and build the artifacts, or work with the teams to build the artifacts and education that deliver it.";
        let report = evaluate_document(
            COVER_LETTER,
            "Dear Acme Hiring Team,\n\nIn my Engineer role I cut reporting costs by 12%.",
            &[claim("Cut reporting costs by 12%")],
            &[],
            Some("Acme"),
            Some("Engineer"),
            &[gap(requirement)],
        );

        assert!(!report.blocking, "findings: {:?}", report.findings);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "gap_not_named" && !finding.blocking),
            "findings: {:?}",
            report.findings
        );
    }

    #[test]
    fn a_skills_list_is_not_checked_as_experience() {
        let draft = "## Skills\n- AI & Process Automation\n- Cross-Functional Collaboration\n";
        let report = evaluate_document(
            RESUME,
            draft,
            &[claim("Delivered work for the team")],
            &[],
            None,
            None,
            &[],
        );

        assert!(report.findings.is_empty(), "findings: {:?}", report.findings);
    }

    #[test]
    fn letter_must_name_the_employer_and_role() {
        let report = evaluate_document(
            COVER_LETTER,
            "I am interested in this position.",
            &[claim("Interested in this position")],
            &[],
            Some("Acme"),
            Some("Engineer"),
            &[],
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "role_company_mismatch"));
    }

    #[test]
    fn reports_merge_without_losing_blocking_state() {
        let clean = evaluate_document(
            COVER_LETTER,
            "Acme needs an Engineer. I delivered work.",
            &[claim("Delivered work")],
            &[],
            Some("Acme"),
            Some("Engineer"),
            &[],
        );
        let blocked = evaluate_document(
            COVER_LETTER,
            "Acme needs an Engineer. I hold a security clearance.",
            &[claim("Delivered work")],
            &[rule("never_claim", "security clearance", "")],
            Some("Acme"),
            Some("Engineer"),
            &[],
        );

        let merged = merge(vec![clean, blocked]);
        assert!(merged.blocking);
        assert_eq!(merged.findings.len(), 1);
    }

    #[test]
    fn grounding_and_gap_helpers_are_deterministic() {
        assert!(is_grounded(
            "Built a telemetry pipeline used by twelve teams",
            &[claim("Built a telemetry pipeline used by twelve teams")]
        ));
        assert!(!is_grounded(
            "Invented a company-wide platform nobody confirmed",
            &[claim("Built a telemetry pipeline used by twelve teams")]
        ));
        assert!(gap_named(
            "I have not administered Kubernetes clusters by name.",
            "Kubernetes cluster administration"
        ));
        assert!(!gap_named(
            "I have relevant experience.",
            "Kubernetes cluster administration"
        ));
    }

    #[test]
    fn weak_provenance_is_advisory_and_limited_to_cited_claims() {
        let mut weak = claim("Built a telemetry pipeline used by twelve teams");
        weak.id = "weak".to_string();
        weak.evidence_type = "unspecified".to_string();
        weak.evidence_sha256.clear();
        weak.verified_by.clear();
        let mut uncited = weak.clone();
        uncited.id = "uncited".to_string();

        let findings = super::weak_provenance_findings(&[weak, uncited], &["weak".to_string()]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "weak_claim_provenance");
        assert!(!findings[0].blocking);
        assert!(findings[0].message.contains("evidence type"));
    }

    #[test]
    fn cited_claim_ids_are_limited_to_claims_in_the_document() {
        let mut first = claim("Built a telemetry pipeline used by twelve teams");
        first.id = "c1".to_string();
        let mut second = claim("Managed a team of five engineers");
        second.id = "c2".to_string();

        let cited = super::cited_claim_ids(
            "## Experience\n- Built a telemetry pipeline used by twelve teams",
            &[first, second],
        );

        assert_eq!(cited, vec!["c1".to_string()]);
    }
}
