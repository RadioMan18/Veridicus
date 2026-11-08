//! Interactive mock interviews.
//!
//! `/mock` used to be a single model call: the command produced one question and
//! then had no session to accept an answer in. This module gives the command a
//! session instead — a grounded question plan, one turn per answer, optional
//! per-answer coaching, and a debrief that closes the run.
//!
//! Everything that can be measured is measured locally: the plan, the delivery
//! metrics and the score fallback never need a model. The model only writes the
//! coaching prose, so a missing or slow model degrades the wording, not the
//! session itself.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::claims::Claim;
use crate::db::get_db_connection;
use crate::guardrails::fetch_confirmed_claims;
use crate::interview_prep::{
    assemble_questions, load_gaps, matrix_signals, read_posting, InterviewQuestion,
};
use crate::keywords;
use crate::ollama::{chat_with_ollama_async, OllamaChatMessage};
use crate::AppState;

const MODE_COACHING: &str = "coaching";
const MODE_SIMULATION: &str = "simulation";

/// Six is about ten minutes of talking, which is the length the practice
/// products converge on for a single sitting.
const DEFAULT_QUESTIONS: usize = 6;
const MIN_QUESTIONS: usize = 3;
const MAX_QUESTIONS: usize = 12;
const MAX_EVIDENCE_CLAIMS: usize = 24;

const FOLLOW_UP_QUESTION: &str = "What was the concrete outcome, and how did you know it worked?";

const SITUATION_HINTS: &[&str] = &[
    "when i",
    "at my",
    "in my",
    "during",
    "last year",
    "the situation",
    "the context",
    "we had",
    "there was",
    "the team was",
    "back in",
];
const TASK_HINTS: &[&str] = &[
    "needed to",
    "had to",
    "my job",
    "responsible for",
    "goal was",
    "asked me",
    "the task",
    "challenge was",
    "i was asked",
    "my role was",
];
const ACTION_HINTS: &[&str] = &[
    "i led",
    "i built",
    "i ran",
    "i created",
    "i designed",
    "i implemented",
    "i organized",
    "i coordinated",
    "i wrote",
    "i managed",
    "i worked",
    "i developed",
    "i set up",
    "i trained",
    "i launched",
    "i analyzed",
    "i reviewed",
    "i migrated",
    "i automated",
    "i introduced",
    "i started",
    "i drove",
    "i owned",
];
const RESULT_HINTS: &[&str] = &[
    "as a result",
    "the result",
    "the outcome",
    "which meant",
    "so that",
    "reduced",
    "increased",
    "improved",
    "saved",
    "cut",
    "grew",
    "delivered",
    "shipped",
    "percent",
    "on time",
    "under budget",
    "closed",
];
/// Typed answers have no "um", so only unambiguous verbal filler is counted.
const FILLERS: &[&str] = &["um", "uh", "erm", "hmm", "you know", "basically", "actually", "literally"];
const HEDGES: &[&str] = &[
    "i think",
    "i guess",
    "i believe",
    "maybe",
    "probably",
    "hopefully",
    "kind of",
    "sort of",
    "not sure",
    "a bit",
    "somewhat",
    "i feel like",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockQuestion {
    /// 1-based position in the run.
    pub index: usize,
    pub group: String,
    pub text: String,
    pub evidence: String,
    pub claim_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockPlan {
    pub role: String,
    pub mode: String,
    pub questions: Vec<MockQuestion>,
    /// How many confirmed claims the questions can draw on.
    pub grounded_in: usize,
    pub posting_available: bool,
    /// A drill replays only the questions the last run scored lowest.
    pub drill: bool,
    /// The session contract shown before the first question.
    pub contract: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StarCoverage {
    pub situation: bool,
    pub task: bool,
    pub action: bool,
    pub result: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnswerMetrics {
    pub words: usize,
    pub wpm: Option<u32>,
    pub fillers: Vec<String>,
    pub filler_count: usize,
    pub hedges: Vec<String>,
    pub star: StarCoverage,
    pub has_metric: bool,
    pub first_person_i: usize,
    pub first_person_we: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockDimension {
    pub name: String,
    pub score: u8,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockFeedback {
    pub overall: u8,
    pub dimensions: Vec<MockDimension>,
    pub strengths: Vec<String>,
    pub fixes: Vec<String>,
    pub priority: String,
    pub stronger_answer: Option<String>,
    pub metrics: AnswerMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockEvaluation {
    /// One-line interviewer reaction.
    pub ack: String,
    /// A single probe when the answer stopped short; the answer is folded in.
    pub follow_up: Option<String>,
    /// `None` while a follow-up is pending, and always `None` in simulation.
    pub feedback: Option<MockFeedback>,
    pub model_note: Option<String>,
}

/// One completed question, as the palette sends it back for the debrief.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MockTurnInput {
    pub question: String,
    #[serde(default)]
    pub group: String,
    pub answer: String,
    #[serde(default)]
    pub follow_up_answer: Option<String>,
    #[serde(default)]
    pub skipped: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct DimensionDraft {
    #[serde(default)]
    name: String,
    #[serde(default)]
    score: u8,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct EvaluationDraft {
    #[serde(default)]
    ack: String,
    #[serde(default)]
    overall: Option<u8>,
    #[serde(default)]
    dimensions: Vec<DimensionDraft>,
    #[serde(default)]
    strengths: Vec<String>,
    #[serde(default)]
    fixes: Vec<String>,
    #[serde(default)]
    priority: String,
    #[serde(default)]
    stronger_answer: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct DebriefDraft {
    #[serde(default)]
    impression: String,
    #[serde(default)]
    next_steps: Vec<String>,
}

fn normalize_mode(raw: Option<&str>) -> String {
    match raw.unwrap_or_default().trim().to_lowercase().as_str() {
        "simulation" | "sim" | "real" | "realistic" | "interview" => MODE_SIMULATION.to_string(),
        _ => MODE_COACHING.to_string(),
    }
}

fn clamp_count(raw: Option<usize>) -> usize {
    raw.unwrap_or(DEFAULT_QUESTIONS).clamp(MIN_QUESTIONS, MAX_QUESTIONS)
}

/// One question from each group in turn, so a short run still tests behaviour,
/// requirements and culture rather than only the first group in the bank.
pub(crate) fn select_questions(bank: &[InterviewQuestion], count: usize) -> Vec<MockQuestion> {
    let groups = ["technical", "behavioral", "culture-fit"];
    let mut buckets: Vec<Vec<&InterviewQuestion>> = groups
        .iter()
        .map(|group| bank.iter().filter(|entry| entry.group == *group).collect())
        .collect();

    let mut picked: Vec<&InterviewQuestion> = Vec::new();
    let mut cursor = 0usize;
    while picked.len() < count && buckets.iter().any(|bucket| !bucket.is_empty()) {
        let slot = cursor % buckets.len();
        if !buckets[slot].is_empty() {
            picked.push(buckets[slot].remove(0));
        }
        cursor += 1;
    }

    picked
        .into_iter()
        .enumerate()
        .map(|(index, question)| MockQuestion {
            index: index + 1,
            group: question.group.clone(),
            text: question.question.clone(),
            evidence: question.evidence.clone(),
            claim_ids: question.claim_ids.clone(),
        })
        .collect()
}

fn says(padded: &str, phrase: &str) -> bool {
    padded.contains(&format!(" {} ", phrase))
}

/// Delivery facts computed from the answer itself, never guessed.
pub(crate) fn measure_answer(answer: &str, elapsed_seconds: Option<u64>) -> AnswerMetrics {
    let tokens = keywords::words(answer);
    let padded = format!(" {} ", tokens.join(" "));
    let words = tokens.len();

    let fillers: Vec<String> = FILLERS
        .iter()
        .filter(|phrase| says(&padded, phrase))
        .map(|phrase| phrase.to_string())
        .collect();
    let hedges: Vec<String> = HEDGES
        .iter()
        .filter(|phrase| says(&padded, phrase))
        .map(|phrase| phrase.to_string())
        .collect();

    let star = StarCoverage {
        situation: SITUATION_HINTS.iter().any(|phrase| says(&padded, phrase)),
        task: TASK_HINTS.iter().any(|phrase| says(&padded, phrase)),
        action: ACTION_HINTS.iter().any(|phrase| says(&padded, phrase)),
        result: RESULT_HINTS.iter().any(|phrase| says(&padded, phrase)),
    };

    let wpm = match elapsed_seconds {
        Some(seconds) if seconds >= 5 => Some(((words as u64 * 60) / seconds).min(400) as u32),
        _ => None,
    };

    AnswerMetrics {
        words,
        wpm,
        filler_count: fillers.len(),
        fillers,
        hedges,
        star,
        has_metric: answer.chars().any(|character| character.is_ascii_digit()),
        first_person_i: tokens.iter().filter(|token| token.as_str() == "i").count(),
        first_person_we: tokens
            .iter()
            .filter(|token| matches!(token.as_str(), "we" | "our" | "us"))
            .count(),
    }
}

/// A short answer or one with no outcome gets exactly one probe. The caller
/// passes the follow-up answer back, so the exchange never loops.
pub(crate) fn needs_follow_up(metrics: &AnswerMetrics) -> bool {
    metrics.words < 25 || !metrics.star.result
}

fn relevance_score(question: &str, evidence: &str, answer: &str) -> (u8, String) {
    let wanted = keywords::significant_words(&format!("{} {}", question, evidence));
    let spoken: Vec<String> = keywords::significant_words(answer);
    if wanted.is_empty() || spoken.is_empty() {
        return (
            2,
            "The question names nothing specific to check, so this is judged on completeness."
                .to_string(),
        );
    }
    let overlap = wanted
        .iter()
        .filter(|word| spoken.iter().any(|said| said == *word))
        .count();
    let ratio = overlap as f32 / wanted.len() as f32;
    let score = if ratio >= 0.35 {
        5
    } else if ratio >= 0.25 {
        4
    } else if ratio >= 0.15 {
        3
    } else if ratio >= 0.05 {
        2
    } else {
        1
    };
    (
        score,
        format!(
            "The answer picked up {} of the question's key terms.",
            overlap
        ),
    )
}

/// A local score, used for simulation debriefs and whenever the model fails.
/// It never invents content: every line comes from the answer's own shape.
pub(crate) fn fallback_feedback(
    question: &str,
    evidence: &str,
    answer: &str,
    metrics: &AnswerMetrics,
) -> MockFeedback {
    let (relevance, relevance_note) = relevance_score(question, evidence, answer);
    let covered = [
        metrics.star.situation,
        metrics.star.task,
        metrics.star.action,
        metrics.star.result,
    ]
    .iter()
    .filter(|present| **present)
    .count() as u8;
    let structure = 1 + covered;

    let mut specificity = match metrics.words {
        0..=19 => 1,
        20..=44 => 2,
        45..=89 => 3,
        _ => 4,
    };
    if metrics.has_metric {
        specificity = (specificity + 1).min(5);
    }

    let mut impact = 1u8;
    if metrics.star.result {
        impact += 2;
    }
    if metrics.has_metric {
        impact += 2;
    }
    let impact = impact.min(5);

    let dimensions = vec![
        MockDimension {
            name: "Relevance".to_string(),
            score: relevance,
            note: relevance_note,
        },
        MockDimension {
            name: "Structure".to_string(),
            score: structure,
            note: format!("{} of the four STAR parts are present.", covered),
        },
        MockDimension {
            name: "Specificity".to_string(),
            score: specificity,
            note: format!(
                "{} words{}.",
                metrics.words,
                if metrics.has_metric {
                    " with a number in them"
                } else {
                    " with no number"
                }
            ),
        },
        MockDimension {
            name: "Impact".to_string(),
            score: impact,
            note: if metrics.star.result {
                "The outcome is stated.".to_string()
            } else {
                "No outcome is stated.".to_string()
            },
        },
    ];

    let total: usize = dimensions.iter().map(|entry| entry.score as usize).sum();
    let overall = ((total * 100) / (dimensions.len() * 5)).min(100) as u8;

    let mut strengths = Vec::new();
    if metrics.star.result {
        strengths.push("The answer reached an outcome instead of stopping at activity.".to_string());
    }
    if metrics.has_metric {
        strengths.push("A number made the scale checkable.".to_string());
    }
    if metrics.star.action {
        strengths.push("The action was yours, which is what STAR answers are scored on.".to_string());
    }
    if metrics.words >= 60 {
        strengths.push("There was enough detail to follow.".to_string());
    }
    if metrics.hedges.is_empty() && metrics.words > 0 {
        strengths.push("No hedging language weakened the claim.".to_string());
    }

    let mut fixes = Vec::new();
    if !metrics.star.situation {
        fixes.push("Set the scene in one sentence: when, where and what was at stake.".to_string());
    }
    if !metrics.star.task {
        fixes.push("Name the task you owned — what you were asked to do.".to_string());
    }
    if !metrics.star.action {
        fixes.push("Say what you did, in the first person; \"we\" hides your part.".to_string());
    }
    if !metrics.star.result {
        fixes.push("End with the result and how you know it happened.".to_string());
    }
    if !metrics.has_metric {
        fixes.push(
            "Add the number the claim already supports, or say plainly that there is not one."
                .to_string(),
        );
    }
    if !metrics.hedges.is_empty() {
        fixes.push(format!(
            "Drop the hedging ({}); state what happened.",
            metrics.hedges.join(", ")
        ));
    }

    let priority_order = [
        ("Impact", impact),
        ("Structure", structure),
        ("Specificity", specificity),
        ("Relevance", relevance),
    ];
    let weakest = priority_order
        .iter()
        .min_by_key(|(_, score)| *score)
        .map(|(name, _)| *name)
        .unwrap_or("Structure");
    let priority = match weakest {
        "Impact" => {
            "Close with the outcome you can evidence: what changed, by how much, and how you knew."
        }
        "Specificity" => {
            "Replace the summary with one concrete example: what you did, with which tools, at what scale."
        }
        "Relevance" => {
            "Answer the question that was asked; the first sentence should name the thing the interviewer asked about."
        }
        _ => {
            "Rebuild the answer as situation, task, action, then result; the action and the result were the thin parts."
        }
    };

    MockFeedback {
        overall,
        dimensions,
        strengths,
        fixes,
        priority: priority.to_string(),
        stronger_answer: Some(
            "Use the STAR shape and keep to facts you can evidence: one sentence of situation, one of task, the actions you took, and the result you can support."
                .to_string(),
        ),
        metrics: metrics.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluation_prompt(
    role: &str,
    question: &str,
    evidence: &str,
    answer: &str,
    follow_up_answer: Option<&str>,
    metrics: &AnswerMetrics,
    claims: &[Claim],
) -> String {
    let mut prompt = format!(
        "## Role\n{}\n\n## Question\n{}\n\n## What a strong answer includes\n{}\n\n## Confirmed claims\nThe candidate may only draw on these facts:\n",
        role, question, evidence
    );
    for claim in claims.iter().take(MAX_EVIDENCE_CLAIMS) {
        prompt.push_str(&format!("- [{}] {}\n", claim.id, claim.confirmed_wording));
    }
    prompt.push_str(&format!("\n## The candidate's answer\n{}\n", answer.trim()));
    if let Some(follow_up) = follow_up_answer {
        if !follow_up.trim().is_empty() {
            prompt.push_str(&format!(
                "\n## Answer to the interviewer's follow-up\n{}\n",
                follow_up.trim()
            ));
        }
    }

    prompt.push_str(&format!(
        "\n## Measured delivery\n{} words{}; STAR parts present: situation {}, task {}, action {}, result {}; number used: {}; hedges: {}; filler: {}.\n",
        metrics.words,
        metrics
            .wpm
            .map(|wpm| format!(", about {} words per minute", wpm))
            .unwrap_or_default(),
        metrics.star.situation,
        metrics.star.task,
        metrics.star.action,
        metrics.star.result,
        metrics.has_metric,
        if metrics.hedges.is_empty() {
            "none".to_string()
        } else {
            metrics.hedges.join(", ")
        },
        if metrics.fillers.is_empty() {
            "none".to_string()
        } else {
            metrics.fillers.join(", ")
        }
    ));

    prompt.push_str(
        "\n## What to return\nScore this answer as a working interviewer would, on four dimensions of 1 to 5: \
Relevance, Structure, Specificity, Impact. Base every rating on the answer above — not on confidence, length or what you assume about the candidate. \
\"ack\" is one short sentence of interviewer reaction. \"priority\" is the single most useful fix, in one sentence. \
\"stronger_answer\" is direction for a stronger answer, not a fabricated story: refer to the candidate's own material and, where relevant, the confirmed claims. \
Never invent achievements, numbers, employers, tools or outcomes for the candidate. Return JSON only: \
{\"ack\":\"...\",\"overall\":0-100,\"dimensions\":[{\"name\":\"Relevance\",\"score\":1-5,\"note\":\"...\"},{\"name\":\"Structure\",\"score\":1-5,\"note\":\"...\"},{\"name\":\"Specificity\",\"score\":1-5,\"note\":\"...\"},{\"name\":\"Impact\",\"score\":1-5,\"note\":\"...\"}],\"strengths\":[\"...\"],\"fixes\":[\"...\"],\"priority\":\"...\",\"stronger_answer\":\"...\"}.",
    );
    prompt
}

#[allow(clippy::too_many_arguments)]
async fn request_feedback(
    model: &str,
    role: &str,
    question: &str,
    evidence: &str,
    answer: &str,
    follow_up_answer: Option<&str>,
    metrics: &AnswerMetrics,
    claims: &[Claim],
    num_ctx: Option<u32>,
) -> Result<EvaluationDraft, String> {
    let system = "You are an interview coach scoring one answer. Base every rating on the candidate's own words. \
Never invent achievements, numbers, employers, tools or outcomes for the candidate. Return JSON only.";
    let messages = vec![
        OllamaChatMessage {
            role: "system".to_string(),
            content: system.to_string(),
        },
        OllamaChatMessage {
            role: "user".to_string(),
            content: evaluation_prompt(
                role,
                question,
                evidence,
                answer,
                follow_up_answer,
                metrics,
                claims,
            ),
        },
    ];
    let response = chat_with_ollama_async(
        model.to_string(),
        messages,
        Some(0.3),
        Some(900),
        Some("json".to_string()),
        num_ctx,
    )
    .await?;
    serde_json::from_str::<EvaluationDraft>(response.message.content.trim())
        .map_err(|error| format!("the model did not return usable feedback JSON: {}", error))
}

fn clean_list(items: Vec<String>) -> Vec<String> {
    let mut cleaned: Vec<String> = Vec::new();
    for item in items {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        if cleaned.iter().any(|entry| entry.eq_ignore_ascii_case(trimmed)) {
            continue;
        }
        cleaned.push(trimmed.to_string());
        if cleaned.len() == 4 {
            break;
        }
    }
    cleaned
}

fn merge_feedback(draft: EvaluationDraft, fallback: MockFeedback) -> MockFeedback {
    let mut dimensions: Vec<MockDimension> = draft
        .dimensions
        .into_iter()
        .filter(|entry| !entry.name.trim().is_empty())
        .map(|entry| MockDimension {
            name: entry.name.trim().to_string(),
            score: entry.score.clamp(1, 5),
            note: entry.note.trim().to_string(),
        })
        .collect();
    if dimensions.len() < 3 {
        dimensions = fallback.dimensions.clone();
    }

    let total: usize = dimensions.iter().map(|entry| entry.score as usize).sum();
    let computed = ((total * 100) / (dimensions.len().max(1) * 5)).min(100) as u8;
    let overall = match draft.overall {
        Some(value) if value <= 100 => value,
        _ => computed,
    };

    let mut feedback = MockFeedback {
        overall,
        dimensions,
        strengths: clean_list(draft.strengths),
        fixes: clean_list(draft.fixes),
        priority: draft.priority.trim().to_string(),
        stronger_answer: Some(draft.stronger_answer.trim().to_string())
            .filter(|text| !text.is_empty()),
        metrics: fallback.metrics.clone(),
    };

    if feedback.strengths.is_empty() {
        feedback.strengths = fallback.strengths;
    }
    if feedback.fixes.is_empty() {
        feedback.fixes = fallback.fixes;
    }
    if feedback.priority.is_empty() {
        feedback.priority = fallback.priority;
    }
    if feedback.stronger_answer.is_none() {
        feedback.stronger_answer = fallback.stronger_answer;
    }
    feedback
}

fn model_name(config: &crate::ollama::OllamaConfig) -> String {
    config
        .deep_model
        .clone()
        .or_else(|| config.fast_model.clone())
        .unwrap_or_else(|| "llama3.2:latest".to_string())
}

fn group_label(group: &str) -> String {
    match group {
        "technical" => "Technical".to_string(),
        "behavioral" => "Behavioral".to_string(),
        "culture-fit" => "Culture fit".to_string(),
        other => other.to_string(),
    }
}

fn dimension_totals(scored: &[(usize, MockFeedback)]) -> (String, String) {
    let mut totals: Vec<(String, usize, usize)> = Vec::new();
    for (_, feedback) in scored {
        for dimension in &feedback.dimensions {
            match totals.iter_mut().find(|(name, _, _)| name == &dimension.name) {
                Some((_, sum, count)) => {
                    *sum += dimension.score as usize;
                    *count += 1;
                }
                None => totals.push((dimension.name.clone(), dimension.score as usize, 1)),
            }
        }
    }
    if totals.is_empty() {
        return (String::new(), String::new());
    }
    totals.sort_by(|a, b| (b.1 as f32 / b.2 as f32).total_cmp(&(a.1 as f32 / a.2 as f32)));
    let strongest = totals.first().map(|entry| entry.0.clone()).unwrap_or_default();
    let weakest = totals.last().map(|entry| entry.0.clone()).unwrap_or_default();
    (strongest, weakest)
}

fn fallback_next_steps(
    role: &str,
    turns: &[MockTurnInput],
    scored: &[(usize, MockFeedback)],
) -> Vec<String> {
    let mut steps = vec![format!(
        "Rehearse the weakest answer again: `/mock {}`",
        role
    )];
    let skipped = turns.iter().filter(|turn| turn.skipped).count();
    if skipped > 0 {
        steps.push(format!(
            "{} question(s) were skipped — those are the gaps to prepare before the real interview.",
            skipped
        ));
    }
    let without_result = scored
        .iter()
        .filter(|(_, feedback)| !feedback.metrics.star.result)
        .count();
    if without_result > 0 {
        steps.push(format!(
            "{} answer(s) stopped before a result; end those with the outcome your claims support.",
            without_result
        ));
    }
    let without_number = scored
        .iter()
        .filter(|(_, feedback)| !feedback.metrics.has_metric)
        .count();
    if without_number > 0 {
        steps.push(format!(
            "{} answer(s) carried no number; use the metrics your confirmed claims already contain.",
            without_number
        ));
    }
    steps
}

pub(crate) fn render_debrief(
    role: &str,
    mode: &str,
    turns: &[MockTurnInput],
    scored: &[(usize, MockFeedback)],
    impression: Option<&str>,
    next_steps: &[String],
) -> String {
    let answered = scored.len();
    let average = if answered == 0 {
        0
    } else {
        scored
            .iter()
            .map(|(_, feedback)| feedback.overall as usize)
            .sum::<usize>()
            / answered
    };
    let mode_label = if mode == MODE_SIMULATION {
        "Simulation"
    } else {
        "Coaching"
    };

    let mut md = format!("# Mock interview debrief — {}\n\n", role);
    md.push_str(&format!(
        "**{} of {} answered** · {} mode · average **{}/100**\n\n",
        answered,
        turns.len(),
        mode_label,
        average
    ));

    md.push_str("## Interviewer's impression\n\n");
    match impression {
        Some(text) if !text.trim().is_empty() => md.push_str(&format!("{}\n\n", text.trim())),
        _ => {
            let (strongest, weakest) = dimension_totals(scored);
            if strongest.is_empty() {
                md.push_str("No answers were scored, so there is nothing to judge yet.\n\n");
            } else {
                md.push_str(&format!(
                    "{} answers averaged {}/100. {} was the steadiest dimension; {} is where the answers thinned out.\n\n",
                    answered, average, strongest, weakest
                ));
            }
        }
    }

    md.push_str("## Scores\n");
    for (index, turn) in turns.iter().enumerate() {
        let label = format!("Question {}", index + 1);
        let group = group_label(&turn.group);
        match scored.iter().find(|(position, _)| *position == index) {
            Some((_, feedback)) => {
                let breakdown: Vec<String> = feedback
                    .dimensions
                    .iter()
                    .map(|dimension| format!("{} {}/5", dimension.name.to_lowercase(), dimension.score))
                    .collect();
                md.push_str(&format!(
                    "- **{} · {}** — {}/100 ({})\n",
                    label,
                    group,
                    feedback.overall,
                    breakdown.join(", ")
                ));
                if !feedback.priority.is_empty() {
                    md.push_str(&format!("  {}\n", feedback.priority));
                }
            }
            None => {
                md.push_str(&format!("- **{} · {}** — skipped\n", label, group));
            }
        }
    }
    md.push('\n');

    let strengths: Vec<String> = clean_list(
        scored
            .iter()
            .flat_map(|(_, feedback)| feedback.strengths.clone())
            .collect(),
    );
    if !strengths.is_empty() {
        md.push_str("## What worked\n");
        for strength in &strengths {
            md.push_str(&format!("- {}\n", strength));
        }
        md.push('\n');
    }

    let fixes: Vec<String> = clean_list(
        scored
            .iter()
            .flat_map(|(_, feedback)| feedback.fixes.clone())
            .collect(),
    );
    if !fixes.is_empty() {
        md.push_str("## Fix these first\n");
        for fix in &fixes {
            md.push_str(&format!("- {}\n", fix));
        }
        md.push('\n');
    }

    if !scored.is_empty() {
        let words: usize = scored
            .iter()
            .map(|(_, feedback)| feedback.metrics.words)
            .sum();
        let fillers: usize = scored
            .iter()
            .map(|(_, feedback)| feedback.metrics.filler_count)
            .sum();
        let hedges: usize = scored
            .iter()
            .map(|(_, feedback)| feedback.metrics.hedges.len())
            .sum();
        let numbers = scored
            .iter()
            .filter(|(_, feedback)| feedback.metrics.has_metric)
            .count();
        let results = scored
            .iter()
            .filter(|(_, feedback)| feedback.metrics.star.result)
            .count();
        md.push_str("## Delivery\n");
        md.push_str(&format!(
            "- {} words across {} answer(s), about {} per answer.\n",
            words,
            answered,
            words / answered.max(1)
        ));
        md.push_str(&format!(
            "- {} answer(s) reached a result and {} carried a number.\n",
            results, numbers
        ));
        md.push_str(&format!(
            "- {} hedge(s) and {} filler word(s) in total.\n\n",
            hedges, fillers
        ));
    }

    let steps = if next_steps.is_empty() {
        fallback_next_steps(role, turns, scored)
    } else {
        next_steps.to_vec()
    };
    if !steps.is_empty() {
        md.push_str("## Next\n");
        for step in &steps {
            md.push_str(&format!("- {}\n", step));
        }
    }

    md
}

/// Build the run: grounded questions, one per turn, from the bank that already
/// exists. No model call, so the session starts instantly and cannot hallucinate
/// a question the ledger cannot support.
#[tauri::command]
pub fn plan_mock_interview(
    role: String,
    application_id: Option<String>,
    mode: Option<String>,
    question_count: Option<usize>,
    state: State<'_, AppState>,
) -> Result<MockPlan, String> {
    let role = role.trim().to_string();
    if role.is_empty() {
        return Err("Give /mock a role title. Example: /mock Director of Training".to_string());
    }

    let grounded = grounded_bank(&state, application_id.as_deref())?;
    let questions = select_questions(&grounded.questions, clamp_count(question_count));
    if questions.is_empty() {
        return Err("No grounded questions could be built from the ledger.".to_string());
    }

    let mode = normalize_mode(mode.as_deref());
    let contract = format!(
        "{} questions · {} · /skip passes · /retry redoes · /end gives the debrief",
        questions.len(),
        if mode == MODE_SIMULATION {
            "simulation: no feedback until the end"
        } else {
            "coaching: feedback after every answer"
        }
    );

    Ok(MockPlan {
        role,
        mode,
        questions,
        grounded_in: grounded.claims,
        posting_available: grounded.posting_available,
        drill: false,
        contract,
    })
}

/// The grounded question bank, rebuilt from the ledger. Both a full run and a
/// drill come through here, so a drilled question carries exactly the wording,
/// evidence and claim ids the full run would have given it.
struct GroundedBank {
    questions: Vec<InterviewQuestion>,
    claims: usize,
    posting_available: bool,
}

fn grounded_bank(
    state: &State<'_, AppState>,
    application_id: Option<&str>,
) -> Result<GroundedBank, String> {
    let db_path = {
        let guard = state.db_path.lock().unwrap();
        guard
            .as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path)?;
    let claims = fetch_confirmed_claims(&conn)?;
    if claims.is_empty() {
        return Err(
            "The mock interview needs confirmed claims to ground its questions. Complete Setup first."
                .to_string(),
        );
    }

    let gaps = match application_id {
        Some(id) => load_gaps(&conn, id)?,
        None => Vec::new(),
    };
    let jd_text = application_id.map(read_posting).unwrap_or_default();
    let (goals, traits) = match application_id {
        Some(id) => matrix_signals(id),
        None => (Vec::new(), Vec::new()),
    };

    let questions = assemble_questions(&jd_text, &goals, &traits, &claims, &gaps);
    Ok(GroundedBank {
        questions,
        claims: claims.len(),
        posting_available: !jd_text.trim().is_empty(),
    })
}

/// Keep only the named questions, in the order given (weakest first), and drop
/// any that the ledger no longer supports rather than inventing a replacement.
fn pick_drill_questions(bank: &[InterviewQuestion], wanted: &[String]) -> Vec<MockQuestion> {
    let mut picked: Vec<MockQuestion> = Vec::new();
    for text in wanted {
        let needle = text.trim();
        if needle.is_empty() {
            continue;
        }
        let found = match bank
            .iter()
            .find(|entry| entry.question.eq_ignore_ascii_case(needle))
        {
            Some(entry) => entry,
            None => continue,
        };
        if picked.iter().any(|entry| entry.text == found.question) {
            continue;
        }
        picked.push(MockQuestion {
            index: picked.len() + 1,
            group: found.group.clone(),
            text: found.question.clone(),
            evidence: found.evidence.clone(),
            claim_ids: found.claim_ids.clone(),
        });
    }
    picked
}

/// A drill: the last run's weakest questions, asked again in coaching mode so
/// each answer is scored and can be compared with the score it has to beat.
#[tauri::command]
pub fn plan_mock_drill(
    role: String,
    application_id: Option<String>,
    questions: Vec<String>,
    state: State<'_, AppState>,
) -> Result<MockPlan, String> {
    let role = role.trim().to_string();
    if role.is_empty() {
        return Err("The drill needs the role it is practising for.".to_string());
    }

    let grounded = grounded_bank(&state, application_id.as_deref())?;
    let picked = pick_drill_questions(&grounded.questions, &questions);
    if picked.is_empty() {
        return Err(
            "None of those questions are in the grounded bank any more. Run a full mock to rebuild the set."
                .to_string(),
        );
    }

    let count = picked.len();
    Ok(MockPlan {
        role,
        mode: MODE_COACHING.to_string(),
        questions: picked,
        grounded_in: grounded.claims,
        posting_available: grounded.posting_available,
        drill: true,
        contract: format!(
            "drill · {} question(s), weakest first · feedback after every answer · repeats until each answer beats its best score",
            count
        ),
    })
}

/// Score one answer. In coaching mode the model writes the coaching prose; in
/// simulation mode nothing is disclosed yet, so no model call is made at all.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn evaluate_mock_answer(
    role: String,
    mode: String,
    question: String,
    evidence: String,
    answer: String,
    follow_up_answer: Option<String>,
    elapsed_seconds: Option<u64>,
    state: State<'_, AppState>,
) -> Result<MockEvaluation, String> {
    if answer.trim().len() < 2 {
        return Err(
            "Answer in your own words — even one sentence works. Use /skip to move on.".to_string(),
        );
    }

    let mode = normalize_mode(Some(&mode));
    let combined = match &follow_up_answer {
        Some(extra) if !extra.trim().is_empty() => {
            format!("{}\n{}", answer.trim(), extra.trim())
        }
        _ => answer.trim().to_string(),
    };
    let metrics = measure_answer(&combined, elapsed_seconds);

    if follow_up_answer.is_none() && needs_follow_up(&metrics) {
        return Ok(MockEvaluation {
            ack: "Thanks — let me press on one detail.".to_string(),
            follow_up: Some(FOLLOW_UP_QUESTION.to_string()),
            feedback: None,
            model_note: None,
        });
    }

    if mode == MODE_SIMULATION {
        return Ok(MockEvaluation {
            ack: "Thanks — noted. Next question.".to_string(),
            follow_up: None,
            feedback: None,
            model_note: None,
        });
    }

    let fallback = fallback_feedback(&question, &evidence, &combined, &metrics);

    let db_path = {
        let guard = state.db_path.lock().unwrap();
        guard
            .as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path)?;
    let claims = fetch_confirmed_claims(&conn)?;
    let config = crate::ollama::get_ollama_config(state.clone()).unwrap_or_default();
    let model = model_name(&config);

    match request_feedback(
        &model,
        &role,
        &question,
        &evidence,
        &answer,
        follow_up_answer.as_deref(),
        &metrics,
        &claims,
        config.num_ctx,
    )
    .await
    {
        Ok(draft) => {
            let ack = if draft.ack.trim().is_empty() {
                "Thanks — here is how that landed.".to_string()
            } else {
                draft.ack.trim().to_string()
            };
            Ok(MockEvaluation {
                ack,
                follow_up: None,
                feedback: Some(merge_feedback(draft, fallback)),
                model_note: None,
            })
        }
        Err(error) => Ok(MockEvaluation {
            ack: "Thanks — here is what I can measure without the model.".to_string(),
            follow_up: None,
            feedback: Some(fallback),
            model_note: Some(format!("Coaching was scored locally: {}", error)),
        }),
    }
}

async fn request_debrief(
    model: &str,
    role: &str,
    turns: &[MockTurnInput],
    scored: &[(usize, MockFeedback)],
    num_ctx: Option<u32>,
) -> Result<DebriefDraft, String> {
    let mut prompt = format!("## Role\n{}\n\n## Transcript\n", role);
    for (index, turn) in turns.iter().enumerate() {
        prompt.push_str(&format!(
            "\n### Question {}\n{}\n\n**Answer:** {}\n",
            index + 1,
            turn.question,
            if turn.skipped {
                "(skipped)"
            } else {
                turn.answer.trim()
            }
        ));
        if let Some(extra) = &turn.follow_up_answer {
            if !extra.trim().is_empty() {
                prompt.push_str(&format!("**Follow-up answer:** {}\n", extra.trim()));
            }
        }
        if let Some((_, feedback)) = scored.iter().find(|(position, _)| *position == index) {
            prompt.push_str(&format!("**Locally measured score:** {}/100\n", feedback.overall));
        }
    }
    prompt.push_str(
        "\n## What to return\nWrite the interviewer's honest closing impression of this run in two to four sentences, naming what specifically helped or hurt the answers. \
Then give three next steps the candidate can act on before the real interview. \
Use only what appears above; never invent employers, numbers, tools or outcomes. \
Return JSON only: {\"impression\":\"...\",\"next_steps\":[\"...\",\"...\",\"...\"]}.",
    );

    let system = "You are an interviewer writing a debrief after a practice run. Be specific, fair and brief. \
Never invent facts about the candidate. Return JSON only.";
    let messages = vec![
        OllamaChatMessage {
            role: "system".to_string(),
            content: system.to_string(),
        },
        OllamaChatMessage {
            role: "user".to_string(),
            content: prompt,
        },
    ];
    let response = chat_with_ollama_async(
        model.to_string(),
        messages,
        Some(0.4),
        Some(700),
        Some("json".to_string()),
        num_ctx,
    )
    .await?;
    serde_json::from_str::<DebriefDraft>(response.message.content.trim())
        .map_err(|error| format!("the model did not return a usable debrief: {}", error))
}

/// Close the run. The local scores are always available; the model only adds the
/// closing impression, so an offline machine still gets a usable debrief.
#[tauri::command]
pub async fn finish_mock_interview(
    role: String,
    mode: Option<String>,
    turns: Vec<MockTurnInput>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    if turns.is_empty() {
        return Err("No answers were recorded, so there is nothing to debrief.".to_string());
    }

    let mut scored: Vec<(usize, MockFeedback)> = Vec::new();
    for (index, turn) in turns.iter().enumerate() {
        if turn.skipped {
            continue;
        }
        let combined = match &turn.follow_up_answer {
            Some(extra) if !extra.trim().is_empty() => {
                format!("{}\n{}", turn.answer.trim(), extra.trim())
            }
            _ => turn.answer.clone(),
        };
        if combined.trim().is_empty() {
            continue;
        }
        let metrics = measure_answer(&combined, None);
        scored.push((
            index,
            fallback_feedback(&turn.question, "", &combined, &metrics),
        ));
    }

    let config = crate::ollama::get_ollama_config(state.clone()).unwrap_or_default();
    let model = model_name(&config);
    let draft = request_debrief(&model, &role, &turns, &scored, config.num_ctx)
        .await
        .unwrap_or_default();

    let impression = Some(draft.impression).filter(|text| !text.trim().is_empty());
    let next_steps = clean_list(draft.next_steps);
    let mode = normalize_mode(mode.as_deref());

    Ok(render_debrief(
        &role,
        &mode,
        &turns,
        &scored,
        impression.as_deref(),
        &next_steps,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        fallback_feedback, measure_answer, needs_follow_up, pick_drill_questions, render_debrief,
        select_questions, MockTurnInput,
    };
    use crate::claims::Claim;
    use crate::interview_prep::assemble_questions;

    fn claim(wording: &str) -> Claim {
        Claim {
            id: "claim-1".to_string(),
            statement: wording.to_string(),
            evidence_source: "master/resume.md".to_string(),
            confirmed_wording: wording.to_string(),
            scope: "as stated".to_string(),
            category: "project".to_string(),
            status: "confirmed".to_string(),
            last_confirmed_at: "2026-11-08".to_string(),
            evidence_type: "unspecified".to_string(),
            evidence_sha256: String::new(),
            verified_by: "ledger-repair".to_string(),
        }
    }

    const STAR_ANSWER: &str = "Last year our onboarding took three weeks. \
I was asked to cut it. I led a working group, rewrote the checklist and trained the managers. \
As a result onboarding dropped to nine days and support tickets fell 40 percent.";

    #[test]
    fn the_plan_covers_every_group_before_repeating_one() {
        let claims = vec![claim("Cut planning time from 8 hours to 30 minutes")];
        let bank = assemble_questions("", &[], &[], &claims, &[]);
        let plan = select_questions(&bank, 6);

        assert_eq!(plan.len(), 6);
        assert_eq!(plan[0].group, "technical");
        assert_eq!(plan[1].group, "behavioral");
        assert_eq!(plan[2].group, "culture-fit");
        assert_eq!(plan[0].index, 1);
        assert_eq!(plan[5].index, 6);
    }

    #[test]
    fn a_drill_keeps_only_the_named_questions_in_order() {
        let claims = vec![claim("Cut planning time from 8 hours to 30 minutes")];
        let bank = assemble_questions("", &[], &[], &claims, &[]);
        let full = select_questions(&bank, 6);
        let wanted = vec![
            full[3].text.clone(),
            "A question that is not in the bank".to_string(),
            full[0].text.clone(),
        ];

        let drill = pick_drill_questions(&bank, &wanted);

        assert_eq!(drill.len(), 2, "unknown questions are dropped, not invented");
        assert_eq!(drill[0].text, full[3].text, "the weakest answer is asked first");
        assert_eq!(drill[1].text, full[0].text);
        assert_eq!(drill[0].index, 1);
        assert_eq!(drill[1].index, 2);
    }

    #[test]
    fn a_one_line_answer_gets_one_follow_up() {
        let metrics = measure_answer("I led a migration once.", Some(12));
        assert!(needs_follow_up(&metrics));
    }

    #[test]
    fn a_full_star_answer_needs_no_follow_up() {
        let metrics = measure_answer(STAR_ANSWER, Some(40));
        assert!(metrics.star.situation, "situation");
        assert!(metrics.star.task, "task");
        assert!(metrics.star.action, "action");
        assert!(metrics.star.result, "result");
        assert!(metrics.has_metric);
        assert!(!needs_follow_up(&metrics));
        // 39 words in 40 seconds.
        assert_eq!(metrics.wpm, Some(58));
    }

    #[test]
    fn hedging_is_measured_without_inventing_filler() {
        let metrics = measure_answer("I think I basically led it, kind of.", None);
        assert!(metrics.hedges.contains(&"i think".to_string()));
        assert!(metrics.hedges.contains(&"kind of".to_string()));
        assert!(!metrics.star.result, "no outcome was stated");
    }

    #[test]
    fn the_local_score_ranks_a_star_answer_above_a_one_liner() {
        let question = "Tell me about a time you led a change.";
        let evidence = "Strong answers cite: \"Cut planning time from 8 hours to 30 minutes\".";

        let weak_metrics = measure_answer("I led a migration once.", None);
        let weak = fallback_feedback(question, evidence, "I led a migration once.", &weak_metrics);

        let strong_metrics = measure_answer(STAR_ANSWER, None);
        let strong = fallback_feedback(question, evidence, STAR_ANSWER, &strong_metrics);

        assert!(strong.overall > weak.overall, "{} vs {}", strong.overall, weak.overall);
        assert!(strong.overall <= 100 && weak.overall <= 100);
        assert!(strong
            .dimensions
            .iter()
            .all(|dimension| (1..=5).contains(&dimension.score)));
        assert!(!strong.fixes.iter().any(|fix| fix.contains("End with the result")));
    }

    #[test]
    fn the_debrief_names_every_question_and_the_next_run() {
        let turns = vec![
            MockTurnInput {
                question: "Tell me about a time you led a change.".to_string(),
                group: "behavioral".to_string(),
                answer: STAR_ANSWER.to_string(),
                follow_up_answer: None,
                skipped: false,
            },
            MockTurnInput {
                question: "What is your approach to a new stack?".to_string(),
                group: "technical".to_string(),
                answer: String::new(),
                follow_up_answer: None,
                skipped: true,
            },
        ];

        let metrics = measure_answer(STAR_ANSWER, None);
        let feedback = fallback_feedback(&turns[0].question, "", STAR_ANSWER, &metrics);
        let scored = vec![(0usize, feedback)];

        let md = render_debrief("Engineer", "coaching", &turns, &scored, None, &[]);

        assert!(md.contains("# Mock interview debrief — Engineer"));
        assert!(md.contains("**1 of 2 answered**"));
        assert!(md.contains("Question 1 · Behavioral"));
        assert!(md.contains("Question 2 · Technical"));
        assert!(md.contains("skipped"));
        assert!(md.contains("/mock Engineer"));
        assert!(md.contains("## Delivery"));
    }
}
