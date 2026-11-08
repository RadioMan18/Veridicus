//! Interview preparation: a question bank and a STAR story bank, both grounded
//! in the confirmed claims ledger.
//!
//! The bank is deterministic so it always exists; only the stories use the model,
//! and a story that cannot be grounded comes back as `needs_story` with a reason
//! instead of prose. `interview-prep.md` is written next to the application's
//! documents, and the stories that cover a gap are recorded on that gap row.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use tauri::State;

use crate::claims::Claim;
use crate::db::get_db_connection;
use crate::guardrails::{cited_claim_ids, fetch_confirmed_claims, is_grounded};
use crate::keywords;
use crate::ollama::{chat_with_ollama_async, model_identity_async, OllamaChatMessage};
use crate::AppState;

/// The eight STAR themes. Every prep carries all eight, including the ones with
/// no story, so the gap is visible rather than absent.
pub const THEMES: [&str; 8] = [
    "leadership",
    "problem-solving",
    "teamwork",
    "conflict",
    "ownership",
    "failure",
    "achievement",
    "adaptability",
];

const QUESTION_GROUPS: [&str; 3] = ["technical", "behavioral", "culture-fit"];
const PER_GROUP: usize = 5;
const MAX_STORY_CLAIMS: usize = 24;
const MAX_JD_BYTES: usize = 8_000;

const REASON_NO_CLAIM: &str = "no confirmed claim touches this theme yet";
const REASON_UNGROUNDED: &str = "the draft could not be grounded in a confirmed claim";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewQuestion {
    pub question: String,
    pub group: String,
    pub evidence: String,
    pub claim_ids: Vec<String>,
    pub gap_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarStory {
    pub theme: String,
    /// `ready` or `needs_story`.
    pub status: String,
    pub situation: String,
    pub task: String,
    pub action: String,
    pub result: String,
    pub claim_ids: Vec<String>,
    pub gap_ids: Vec<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewPrep {
    pub application_id: String,
    pub employer: String,
    pub role: String,
    pub generated_at: String,
    pub model: Option<String>,
    pub model_digest: Option<String>,
    pub questions: Vec<InterviewQuestion>,
    pub stories: Vec<StarStory>,
    pub warnings: Vec<String>,
}

/// A story draft as the model returns it.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoryDraft {
    #[serde(default)]
    theme: String,
    #[serde(default)]
    situation: String,
    #[serde(default)]
    task: String,
    #[serde(default)]
    action: String,
    #[serde(default)]
    result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoryDraftResponse {
    #[serde(default)]
    stories: Vec<StoryDraft>,
}

/// A requirement row as the prep needs it.
#[derive(Debug, Clone)]
struct GapInfo {
    id: String,
    requirement: String,
}

fn story_text(draft: &StoryDraft) -> String {
    format!(
        "Situation: {} Task: {} Action: {} Result: {}",
        draft.situation.trim(),
        draft.task.trim(),
        draft.action.trim(),
        draft.result.trim()
    )
}

/// Digit runs in a text, normalized so "007" and "7" compare equal.
fn numeric_tokens(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_ascii_digit())
        .filter(|token| !token.is_empty())
        .map(|token| token.trim_start_matches('0').to_string())
        .filter(|token| !token.is_empty())
        .collect()
}

/// Every number in `text` must appear in a confirmed claim.
fn digits_are_supported(text: &str, claims: &[Claim]) -> bool {
    let allowed: Vec<String> = claims
        .iter()
        .flat_map(|claim| {
            let mut numbers = numeric_tokens(&claim.confirmed_wording);
            numbers.extend(numeric_tokens(&claim.statement));
            numbers
        })
        .collect();
    numeric_tokens(text)
        .iter()
        .all(|token| allowed.iter().any(|candidate| candidate == token))
}

fn theme_keywords(theme: &str) -> &'static [&'static str] {
    match theme {
        "leadership" => &["led", "lead", "managed", "mentor", "coached", "directed"],
        "problem-solving" => &["solved", "diagnosed", "reduced", "fixed", "analyzed", "resolved"],
        "teamwork" => &["team", "collaborat", "partner", "cross-functional"],
        "conflict" => &["conflict", "disagree", "negotiat", "stakeholder", "escalat"],
        "ownership" => &["owned", "accountab", "launched", "delivered", "built"],
        "failure" => &["failed", "lesson", "learned", "setback", "mistake"],
        "achievement" => &["increased", "improved", "cut", "grew", "award", "saved"],
        "adaptability" => &["adapt", "migrat", "transition", "change", "pivot"],
        _ => &[],
    }
}

fn claim_touches_theme(claim: &Claim, theme: &str) -> bool {
    let text = keywords::claim_text_of(claim).to_lowercase();
    theme_keywords(theme)
        .iter()
        .any(|keyword| text.contains(keyword))
}

fn claims_touching_theme<'a>(claims: &'a [Claim], theme: &str) -> Vec<&'a Claim> {
    claims
        .iter()
        .filter(|claim| claim_touches_theme(claim, theme))
        .collect()
}

/// Gaps whose requirement shares at least two significant words with the story.
fn linked_gap_ids(text: &str, gaps: &[GapInfo]) -> Vec<String> {
    let story_words = keywords::significant_words(text);
    gaps.iter()
        .filter(|gap| {
            let requirement_words = keywords::significant_words(&gap.requirement);
            let overlap = requirement_words
                .iter()
                .filter(|word| story_words.iter().any(|story_word| story_word == *word))
                .count();
            overlap >= 2
        })
        .map(|gap| gap.id.clone())
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn push_question(
    questions: &mut Vec<InterviewQuestion>,
    seen: &mut HashSet<String>,
    group: &str,
    question: String,
    evidence: String,
    claim_ids: Vec<String>,
    gap_ids: Vec<String>,
) {
    let key = question.to_lowercase();
    if !seen.insert(key) {
        return;
    }
    questions.push(InterviewQuestion {
        question,
        group: group.to_string(),
        evidence,
        claim_ids,
        gap_ids,
    });
}

/// Build the bank deterministically: three groups, five prompts each.
pub(crate) fn assemble_questions(
    jd_text: &str,
    goals: &[String],
    traits: &[String],
    claims: &[Claim],
    gaps: &[GapInfo],
) -> Vec<InterviewQuestion> {
    let mut questions = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Technical: the requirements the posting names, then the confirmed claims.
    for gap in gaps.iter().take(PER_GROUP) {
        push_question(
            &mut questions,
            &mut seen,
            "technical",
            format!("Walk me through your experience with: {}.", gap.requirement),
            format!(
                "No confirmed claim matches this requirement yet. The gap is recorded as {}.",
                gap.id
            ),
            Vec::new(),
            vec![gap.id.clone()],
        );
    }
    for claim in claims.iter().take(PER_GROUP) {
        push_question(
            &mut questions,
            &mut seen,
            "technical",
            format!(
                "Tell me how you did this, in concrete terms: {}.",
                claim.confirmed_wording.trim_end_matches('.')
            ),
            "Answer from this confirmed claim; do not add numbers or tools it does not contain."
                .to_string(),
            vec![claim.id.clone()],
            Vec::new(),
        );
    }

    // Behavioral: one prompt per theme, with the claims that can support it.
    for theme in THEMES {
        let supporting = claims_touching_theme(claims, theme);
        let claim_ids: Vec<String> = supporting.iter().map(|claim| claim.id.clone()).collect();
        let evidence = match supporting.first() {
            Some(claim) => format!(
                "Strong answers cite: \"{}\".",
                claim.confirmed_wording.trim_end_matches('.')
            ),
            None => format!(
                "No confirmed story yet for this theme ({}). Say so or build one.",
                REASON_NO_CLAIM
            ),
        };
        push_question(
            &mut questions,
            &mut seen,
            "behavioral",
            format!("Tell me about a time your work showed {}.", theme.replace('-', " ")),
            evidence,
            claim_ids,
            Vec::new(),
        );
    }

    // Culture fit: what the posting says it is hiring for.
    for goal in goals.iter().take(3) {
        push_question(
            &mut questions,
            &mut seen,
            "culture-fit",
            format!("This role exists to {}. How would you approach it?", goal.to_lowercase()),
            "Frame the answer around evidence you can support, not enthusiasm.".to_string(),
            Vec::new(),
            Vec::new(),
        );
    }
    for value in traits.iter().take(2) {
        push_question(
            &mut questions,
            &mut seen,
            "culture-fit",
            format!("The posting values \"{}\". Where has that shown up in your work?", value),
            "Answer with a confirmed example.".to_string(),
            Vec::new(),
            Vec::new(),
        );
    }
    if questions.iter().filter(|entry| entry.group == "culture-fit").count() < PER_GROUP {
        let sentence = jd_text
            .lines()
            .map(str::trim)
            .find(|line| line.len() > 20 && !line.starts_with('#'))
            .map(|line| line.chars().take(160).collect::<String>());
        if let Some(sentence) = sentence {
            push_question(
                &mut questions,
                &mut seen,
                "culture-fit",
                format!(
                    "The posting says: \"{}\". Which part of that is closest to your confirmed experience?",
                    sentence.trim_end_matches('.')
                ),
                "Pick the part you can evidence; name the rest as something to learn.".to_string(),
                Vec::new(),
                Vec::new(),
            );
        }
    }

    // Fill any short group so the counts always exist. These carry no facts.
    let padding: [(&str, &str); 3] = [
        (
            "technical",
            "Which part of this role's technical scope would you need to learn first, and how would you learn it?",
        ),
        (
            "behavioral",
            "Tell me about a decision you made with incomplete information.",
        ),
        (
            "culture-fit",
            "What kind of working environment brings out your best work?",
        ),
    ];
    for (group, question) in padding {
        let mut count = questions.iter().filter(|entry| entry.group == group).count();
        while count < PER_GROUP {
            push_question(
                &mut questions,
                &mut seen,
                group,
                format!("{} (#{})", question, count + 1),
                "No posting-specific evidence; answer from confirmed claims.".to_string(),
                Vec::new(),
                Vec::new(),
            );
            count = questions.iter().filter(|entry| entry.group == group).count();
        }
    }

    questions
}

/// Validate one draft against the ledger. Returns the ready story or a reason.
fn validate_story(
    draft: &StoryDraft,
    claims: &[Claim],
    gaps: &[GapInfo],
) -> Result<StarStory, String> {
    let text = story_text(draft);
    if draft.situation.trim().len() < 10 || draft.action.trim().len() < 10 {
        return Err(REASON_UNGROUNDED.to_string());
    }
    if !digits_are_supported(&text, claims) {
        return Err("the draft contains a number no confirmed claim supports".to_string());
    }
    if !is_grounded(&text, claims) {
        return Err(REASON_UNGROUNDED.to_string());
    }
    let claim_ids = cited_claim_ids(&text, claims);
    if claim_ids.is_empty() {
        return Err(REASON_UNGROUNDED.to_string());
    }

    Ok(StarStory {
        theme: draft.theme.clone(),
        status: "ready".to_string(),
        situation: draft.situation.trim().to_string(),
        task: draft.task.trim().to_string(),
        action: draft.action.trim().to_string(),
        result: draft.result.trim().to_string(),
        claim_ids,
        gap_ids: linked_gap_ids(&text, gaps),
        reason: None,
    })
}

/// Turn model drafts into exactly eight themed stories.
pub(crate) fn assemble_stories(
    drafts: &[StoryDraft],
    claims: &[Claim],
    gaps: &[GapInfo],
) -> Vec<StarStory> {
    THEMES
        .iter()
        .map(|theme| {
            let theme = theme.to_string();
            let draft = drafts
                .iter()
                .find(|draft| draft.theme.to_lowercase() == theme);
            let empty = |reason: String| StarStory {
                theme: theme.clone(),
                status: "needs_story".to_string(),
                situation: String::new(),
                task: String::new(),
                action: String::new(),
                result: String::new(),
                claim_ids: Vec::new(),
                gap_ids: Vec::new(),
                reason: Some(reason),
            };

            match draft {
                Some(draft) => match validate_story(draft, claims, gaps) {
                    Ok(mut story) => {
                        story.theme = theme.clone();
                        story
                    }
                    Err(reason) => empty(reason),
                },
                None => {
                    let reason = if claims_touching_theme(claims, &theme).is_empty() {
                        REASON_NO_CLAIM.to_string()
                    } else {
                        "the model returned no draft for this theme".to_string()
                    };
                    empty(reason)
                }
            }
        })
        .collect()
}

fn render_markdown(prep: &InterviewPrep) -> String {
    let mut md = String::new();
    md.push_str(&format!(
        "# Interview prep — {} — {}\n\n",
        prep.employer, prep.role
    ));
    md.push_str(&format!("Generated {}.\n\n", prep.generated_at));

    md.push_str("## Question bank\n\n");
    for group in QUESTION_GROUPS {
        let questions: Vec<&InterviewQuestion> = prep
            .questions
            .iter()
            .filter(|question| question.group == group)
            .collect();
        md.push_str(&format!(
            "### {} ({})\n\n",
            title_case(group),
            questions.len()
        ));
        for question in questions {
            md.push_str(&format!("- {}\n", question.question));
            md.push_str(&format!(
                "  - What a strong answer includes: {}\n",
                question.evidence
            ));
            if !question.claim_ids.is_empty() {
                md.push_str(&format!("  - Claims: {}\n", question.claim_ids.join(", ")));
            }
            if !question.gap_ids.is_empty() {
                md.push_str(&format!("  - Gaps: {}\n", question.gap_ids.join(", ")));
            }
        }
        md.push('\n');
    }

    md.push_str("## STAR bank\n\n");
    for story in &prep.stories {
        md.push_str(&format!(
            "### {} — {}\n\n",
            title_case(&story.theme),
            story.status
        ));
        if story.status == "ready" {
            md.push_str(&format!("- **Situation:** {}\n", story.situation));
            md.push_str(&format!("- **Task:** {}\n", story.task));
            md.push_str(&format!("- **Action:** {}\n", story.action));
            md.push_str(&format!("- **Result:** {}\n", story.result));
            md.push_str(&format!("- **Claims:** {}\n", story.claim_ids.join(", ")));
            if !story.gap_ids.is_empty() {
                md.push_str(&format!("- **Covers gaps:** {}\n", story.gap_ids.join(", ")));
            }
        } else {
            md.push_str(&format!(
                "No story yet: {}.\n",
                story.reason.as_deref().unwrap_or(REASON_NO_CLAIM)
            ));
        }
        md.push('\n');
    }

    if !prep.warnings.is_empty() {
        md.push_str("## Notes\n\n");
        for warning in &prep.warnings {
            md.push_str(&format!("- {}\n", warning));
        }
    }

    md
}

fn title_case(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), characters.as_str()),
        None => String::new(),
    }
}

fn story_draft_prompt(
    role: &str,
    jd_text: &str,
    claims: &[Claim],
    gaps: &[GapInfo],
    rejected: &[StarStory],
) -> String {
    let mut prompt = format!(
        "Role: {}\n\n## Posting\n{}\n\n## Confirmed claims\n",
        role, jd_text
    );
    for claim in claims.iter().take(MAX_STORY_CLAIMS) {
        prompt.push_str(&format!("- [{}] {}\n", claim.id, claim.confirmed_wording));
    }
    if !gaps.is_empty() {
        prompt.push_str("\n## Gaps the candidate has\n");
        for gap in gaps.iter().take(12) {
            prompt.push_str(&format!("- [{}] {}\n", gap.id, gap.requirement));
        }
    }
    prompt.push_str(
        "\nReturn JSON only: {\"stories\":[{\"theme\":\"...\",\"situation\":\"...\",\"task\":\"...\",\"action\":\"...\",\"result\":\"...\"}]}. \
Write one story per theme: leadership, problem-solving, teamwork, conflict, ownership, failure, achievement, adaptability. \
Every sentence must be supported by a claim above; do not add numbers, tools, employers or outcomes that are not in the claims. \
If a theme has no supporting claim, omit it rather than inventing a story.",
    );
    if !rejected.is_empty() {
        prompt.push_str("\n## Rejected drafts and why\n");
        for story in rejected {
            prompt.push_str(&format!(
                "- {}: {}\n",
                story.theme,
                story.reason.as_deref().unwrap_or(REASON_UNGROUNDED)
            ));
        }
    }
    prompt
}

async fn request_story_drafts(
    model: &str,
    role: &str,
    jd_text: &str,
    claims: &[Claim],
    gaps: &[GapInfo],
    rejected: &[StarStory],
    num_ctx: Option<u32>,
) -> Result<Vec<StoryDraft>, String> {
    let system = "You write STAR interview stories strictly from a supplied claims ledger. \
Never invent facts, numbers, employers or outcomes. Return JSON only.";
    let messages = vec![
        OllamaChatMessage {
            role: "system".to_string(),
            content: system.to_string(),
        },
        OllamaChatMessage {
            role: "user".to_string(),
            content: story_draft_prompt(role, jd_text, claims, gaps, rejected),
        },
    ];
    let response = chat_with_ollama_async(
        model.to_string(),
        messages,
        Some(0.4),
        Some(2000),
        Some("json".to_string()),
        num_ctx,
    )
    .await?;
    let parsed: StoryDraftResponse = serde_json::from_str(response.message.content.trim())
        .map_err(|error| format!("the model did not return usable story JSON: {}", error))?;
    Ok(parsed.stories)
}

fn load_gaps(conn: &rusqlite::Connection, application_id: &str) -> Result<Vec<GapInfo>, String> {
    let mut statement = conn
        .prepare(
            "SELECT id, requirement FROM gaps WHERE application_id = ?1 ORDER BY requirement",
        )
        .map_err(|error| format!("Failed to prepare gap query: {}", error))?;
    let gaps = statement
        .query_map([application_id], |row| {
            Ok(GapInfo {
                id: row.get(0)?,
                requirement: row.get(1)?,
            })
        })
        .map_err(|error| format!("Failed to query gaps: {}", error))?
        .filter_map(|row| row.ok())
        .collect();
    Ok(gaps)
}

fn load_application(
    conn: &rusqlite::Connection,
    application_id: &str,
) -> Result<(String, String), String> {
    conn.query_row(
        "SELECT employer, role FROM application_records WHERE id = ?1",
        [application_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )
    .map_err(|error| format!("Failed to load application {}: {}", application_id, error))
}

fn application_dir(application_id: &str) -> Result<std::path::PathBuf, String> {
    let docs = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    Ok(docs
        .join("CareerOS")
        .join("applications")
        .join(application_id))
}

fn read_posting(application_id: &str) -> String {
    let Ok(dir) = application_dir(application_id) else {
        return String::new();
    };
    let mut text = String::new();
    for name in ["job-description.md", "job-requirements.md"] {
        if let Ok(content) = fs::read_to_string(dir.join(name)) {
            text.push_str(&content);
            text.push_str("\n\n");
        }
    }
    let mut end = text.len().min(MAX_JD_BYTES);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text
}

/// Hiring goals and standout traits from the fit matrix, when it exists.
fn matrix_signals(application_id: &str) -> (Vec<String>, Vec<String>) {
    let Ok(dir) = application_dir(application_id) else {
        return (Vec::new(), Vec::new());
    };
    let Ok(text) = fs::read_to_string(dir.join("fit-matrix.json")) else {
        return (Vec::new(), Vec::new());
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return (Vec::new(), Vec::new());
    };
    let collect = |key: &str| -> Vec<String> {
        value[key]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    (collect("hiring_goals"), collect("standout_traits"))
}

/// Record which stories cover which gaps, so the fit matrix stops showing
/// "no story yet" for a requirement a prepared story addresses.
fn sync_gap_story_ids(conn: &rusqlite::Connection, prep: &InterviewPrep) -> Result<(), String> {
    for story in prep.stories.iter().filter(|story| story.status == "ready") {
        for gap_id in &story.gap_ids {
            let existing: String = conn
                .query_row("SELECT story_ids FROM gaps WHERE id = ?1", [gap_id], |row| {
                    row.get(0)
                })
                .unwrap_or_default();
            let mut ids: Vec<String> = existing
                .split(',')
                .map(|id| id.trim().to_string())
                .filter(|id| !id.is_empty())
                .collect();
            let story_key = format!("{}:{}", prep.application_id, story.theme);
            if !ids.iter().any(|id| id == &story_key) {
                ids.push(story_key);
                conn.execute(
                    "UPDATE gaps SET story_ids = ?1 WHERE id = ?2",
                    rusqlite::params![ids.join(","), gap_id],
                )
                .map_err(|error| format!("Failed to record story ids: {}", error))?;
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn generate_interview_prep(
    application_id: String,
    state: State<'_, AppState>,
) -> Result<InterviewPrep, String> {
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
        return Err("No confirmed claims. Complete Setup so stories can be grounded.".to_string());
    }

    let (employer, role) = load_application(&conn, &application_id)?;
    let gaps = load_gaps(&conn, &application_id)?;
    let jd_text = read_posting(&application_id);
    let (goals, traits) = matrix_signals(&application_id);

    let config = crate::ollama::get_ollama_config(state.clone()).unwrap_or_default();
    let model = config
        .deep_model
        .clone()
        .or_else(|| config.fast_model.clone())
        .unwrap_or_else(|| "llama3.2:latest".to_string());

    let mut warnings = Vec::new();
    let drafts = match request_story_drafts(
        &model,
        &role,
        &jd_text,
        &claims,
        &gaps,
        &[],
        config.num_ctx,
    )
    .await
    {
        Ok(drafts) => drafts,
        Err(error) => {
            warnings.push(format!("Story drafting did not run: {}", error));
            Vec::new()
        }
    };

    let mut stories = assemble_stories(&drafts, &claims, &gaps);
    let retryable: Vec<StarStory> = stories
        .iter()
        .filter(|story| story.status != "ready")
        .filter(|story| story.reason.as_deref() != Some(REASON_NO_CLAIM))
        .cloned()
        .collect();

    if !retryable.is_empty() && !drafts.is_empty() {
        if let Ok(second) = request_story_drafts(
            &model,
            &role,
            &jd_text,
            &claims,
            &gaps,
            &retryable,
            config.num_ctx,
        )
        .await
        {
            let improved = assemble_stories(&second, &claims, &gaps);
            for story in stories.iter_mut().filter(|story| story.status != "ready") {
                if let Some(replacement) = improved
                    .iter()
                    .find(|candidate| candidate.theme == story.theme && candidate.status == "ready")
                {
                    *story = replacement.clone();
                }
            }
        } else {
            warnings.push("A second story attempt failed; the irreplaceable themes stay marked needs_story.".to_string());
        }
    }

    let questions = assemble_questions(&jd_text, &goals, &traits, &claims, &gaps);
    let digest = model_identity_async(model.clone()).await.digest;

    let prep = InterviewPrep {
        application_id: application_id.clone(),
        employer,
        role,
        generated_at: Utc::now().to_rfc3339(),
        model: Some(model),
        model_digest: digest,
        questions,
        stories,
        warnings,
    };

    let dir = application_dir(&application_id)?;
    fs::create_dir_all(&dir).map_err(|error| format!("Failed to create app dir: {}", error))?;
    fs::write(
        dir.join("interview-prep.json"),
        serde_json::to_string_pretty(&prep).unwrap_or_default(),
    )
    .map_err(|error| format!("Failed to write interview-prep.json: {}", error))?;
    fs::write(dir.join("interview-prep.md"), render_markdown(&prep))
        .map_err(|error| format!("Failed to write interview-prep.md: {}", error))?;

    sync_gap_story_ids(&conn, &prep)?;
    Ok(prep)
}

#[tauri::command]
pub fn get_interview_prep(application_id: String) -> Result<InterviewPrep, String> {
    let dir = application_dir(&application_id)?;
    let text = fs::read_to_string(dir.join("interview-prep.json"))
        .map_err(|_| "No interview prep has been generated for this application yet.".to_string())?;
    serde_json::from_str(&text).map_err(|error| format!("Failed to read interview prep: {}", error))
}

/// The saved markdown, for the palette's `/questions` and `/stories`.
#[tauri::command]
pub fn get_interview_prep_markdown(application_id: String) -> Result<String, String> {
    let dir = application_dir(&application_id)?;
    fs::read_to_string(dir.join("interview-prep.md"))
        .map_err(|_| "No interview prep has been generated for this application yet.".to_string())
}

/// Messages for a grounded mock interview.
///
/// The interview cannot run without confirmed claims: a role-playing model with
/// nothing to ground on invents a candidate. History is kept verbatim (bounded)
/// so the exchange is genuinely multi-turn.
pub(crate) fn build_mock_messages(
    role: &str,
    history: &[OllamaChatMessage],
    claims: &[Claim],
    jd_text: &str,
) -> Result<Vec<OllamaChatMessage>, String> {
    if claims.is_empty() {
        return Err(
            "The mock interview needs confirmed claims to ground its questions. Complete Setup first."
                .to_string(),
        );
    }

    let mut system = format!(
        "You are a hiring manager interviewing for the {} role using the STAR method. Ask one question at a time and wait for the answer. \
Ground every follow-up in the confirmed claims and posting below; never invent facts about the candidate or the employer. \
After the candidate answers, give one short piece of feedback, then ask the next question. Stop after fifteen questions.",
        role
    );
    system.push_str("\n\n## Confirmed claims\n");
    for claim in claims.iter().take(MAX_STORY_CLAIMS) {
        system.push_str(&format!("- [{}] {}\n", claim.id, claim.confirmed_wording));
    }
    if !jd_text.trim().is_empty() {
        system.push_str("\n## Posting\n");
        system.push_str(jd_text);
    }

    let mut messages = Vec::with_capacity(history.len() + 2);
    messages.push(OllamaChatMessage {
        role: "system".to_string(),
        content: system,
    });
    let start = history.len().saturating_sub(20);
    messages.extend(history[start..].iter().cloned());
    messages.push(OllamaChatMessage {
        role: "user".to_string(),
        content: if history.is_empty() {
            "Begin the interview with your first question.".to_string()
        } else {
            "Ask the next question, based on my answer above.".to_string()
        },
    });

    Ok(messages)
}

#[tauri::command]
pub async fn start_mock_interview(
    role: String,
    application_id: Option<String>,
    history: Option<Vec<OllamaChatMessage>>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let db_path = {
        let guard = state.db_path.lock().unwrap();
        guard
            .as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path)?;
    let claims = fetch_confirmed_claims(&conn)?;
    let jd_text = application_id
        .as_deref()
        .map(read_posting)
        .unwrap_or_default();

    let messages = build_mock_messages(&role, &history.unwrap_or_default(), &claims, &jd_text)?;
    let config = crate::ollama::get_ollama_config(state.clone()).unwrap_or_default();
    let model = config
        .deep_model
        .clone()
        .or_else(|| config.fast_model.clone())
        .unwrap_or_else(|| "llama3.2:latest".to_string());
    let response = chat_with_ollama_async(
        model,
        messages,
        Some(0.5),
        Some(600),
        None,
        config.num_ctx,
    )
    .await?;

    Ok(response.message.content)
}

#[cfg(test)]
mod tests {
    use super::{
        assemble_questions, assemble_stories, build_mock_messages, numeric_tokens,
        sync_gap_story_ids, GapInfo, InterviewPrep, StarStory, StoryDraft, THEMES,
    };
    use rusqlite::Connection;
    use crate::claims::Claim;
    use crate::ollama::OllamaChatMessage;

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

    fn gap(requirement: &str) -> GapInfo {
        GapInfo {
            id: "gap-1".to_string(),
            requirement: requirement.to_string(),
        }
    }

    fn draft(theme: &str, text: &str) -> StoryDraft {
        StoryDraft {
            theme: theme.to_string(),
            situation: format!("Situation: {}", text),
            task: "Task: own the outcome".to_string(),
            action: format!("Action: {}", text),
            result: "Result: delivered".to_string(),
        }
    }

    const PLANNING_CLAIM: &str = "Cut planning time from 8 hours to 30 minutes";

    #[test]
    fn the_bank_always_has_all_three_groups() {
        let questions = assemble_questions("", &[], &[], &[], &[]);
        for group in ["technical", "behavioral", "culture-fit"] {
            let count = questions.iter().filter(|entry| entry.group == group).count();
            assert!(count >= 5, "group {} has {}", group, count);
        }
        assert!(questions
            .iter()
            .all(|entry| ["technical", "behavioral", "culture-fit"].contains(&entry.group.as_str())));
    }

    #[test]
    fn the_star_bank_always_has_eight_themes() {
        let stories = assemble_stories(&[], &[], &[]);
        assert_eq!(stories.len(), 8);
        for theme in THEMES {
            let story = stories
                .iter()
                .find(|story| story.theme == theme)
                .expect("every theme is present");
            assert_eq!(story.status, "needs_story");
            assert!(story.reason.is_some());
        }
    }

    #[test]
    fn an_ungrounded_story_is_rejected() {
        let stories = assemble_stories(
            &[draft("leadership", "Managed a team that shipped a quantum platform")],
            &[claim(PLANNING_CLAIM)],
            &[],
        );
        let leadership = stories
            .iter()
            .find(|story| story.theme == "leadership")
            .unwrap();
        assert_eq!(leadership.status, "needs_story");
        assert!(leadership.claim_ids.is_empty());
    }

    #[test]
    fn a_story_may_not_add_a_number() {
        let claims = vec![claim(PLANNING_CLAIM)];
        let stories = assemble_stories(
            &[draft(
                "achievement",
                "Cut planning time by 87 percent across the team",
            )],
            &claims,
            &[],
        );
        let story = stories
            .iter()
            .find(|story| story.theme == "achievement")
            .unwrap();
        assert_eq!(story.status, "needs_story");
        assert!(story
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("number"));
    }

    #[test]
    fn a_grounded_story_cites_its_claim() {
        let claims = vec![claim(PLANNING_CLAIM)];
        let stories = assemble_stories(
            &[draft(
                "achievement",
                "Cut planning time from 8 hours to 30 minutes for the learning team",
            )],
            &claims,
            &[gap("Own the learning technology roadmap")],
        );
        let story = stories
            .iter()
            .find(|story| story.theme == "achievement")
            .unwrap();
        assert_eq!(story.status, "ready", "reason: {:?}", story.reason);
        assert_eq!(story.claim_ids, vec!["claim-1".to_string()]);
    }

    #[test]
    fn a_gap_story_link_needs_shared_words() {
        let claims = vec![claim(PLANNING_CLAIM)];
        let stories = assemble_stories(
            &[draft(
                "achievement",
                "Cut planning time from 8 hours to 30 minutes for the learning team",
            )],
            &claims,
            &[gap("Technology roadmap ownership")],
        );
        let story = stories
            .iter()
            .find(|story| story.theme == "achievement")
            .unwrap();
        assert!(
            story.gap_ids.is_empty(),
            "a different requirement must not be claimed as covered: {:?}",
            story.gap_ids
        );
    }

    #[test]
    fn the_mock_requires_claims_and_keeps_history() {
        assert!(build_mock_messages("Engineer", &[], &[], "").is_err());

        let history = vec![
            OllamaChatMessage {
                role: "assistant".to_string(),
                content: "Tell me about a time you led a project.".to_string(),
            },
            OllamaChatMessage {
                role: "user".to_string(),
                content: "I led the migration.".to_string(),
            },
        ];
        let messages =
            build_mock_messages("Engineer", &history, &[claim("Led the migration")], "")
                .expect("grounded");

        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("Led the migration"));
        assert_eq!(messages[1].content, "Tell me about a time you led a project.");
        assert_eq!(messages[2].content, "I led the migration.");
        assert_eq!(messages.len(), 4);
    }

    #[test]
    fn digits_are_compared_by_value() {
        assert_eq!(numeric_tokens("from 08 hours to 30"), vec!["8", "30"]);
    }

    #[test]
    fn a_covered_gap_records_the_story() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE gaps (id TEXT PRIMARY KEY, story_ids TEXT DEFAULT '');\n             INSERT INTO gaps (id, story_ids) VALUES ('gap-1', '');",
        )
        .unwrap();

        let prep = InterviewPrep {
            application_id: "app-1".to_string(),
            employer: "Aurora".to_string(),
            role: "L&D Lead".to_string(),
            generated_at: "2026-11-08T00:00:00Z".to_string(),
            model: None,
            model_digest: None,
            questions: Vec::new(),
            stories: vec![StarStory {
                theme: "achievement".to_string(),
                status: "ready".to_string(),
                situation: String::new(),
                task: String::new(),
                action: String::new(),
                result: String::new(),
                claim_ids: vec!["claim-1".to_string()],
                gap_ids: vec!["gap-1".to_string()],
                reason: None,
            }],
            warnings: Vec::new(),
        };

        sync_gap_story_ids(&conn, &prep).unwrap();
        let stored: String = conn
            .query_row("SELECT story_ids FROM gaps WHERE id = 'gap-1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stored, "app-1:achievement");

        // Re-running must not duplicate the same story on the gap.
        sync_gap_story_ids(&conn, &prep).unwrap();
        let again: String = conn
            .query_row("SELECT story_ids FROM gaps WHERE id = 'gap-1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(again, "app-1:achievement");
    }
}
