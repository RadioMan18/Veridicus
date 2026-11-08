//! Application pack (prompt 10).
//!
//! The pack is an assembly, not a regeneration: the resume and cover letter are
//! copied from the approved artifacts, and a section that does not exist is
//! named as missing with the command that produces it.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use tauri::State;

use crate::claims::Claim;
use crate::db::get_db_connection;
use crate::fact_check::verify_metrics;
use crate::guardrails::{cited_claim_ids, fetch_confirmed_claims};
use crate::keywords;
use crate::ollama::{chat_with_ollama_async, OllamaChatMessage};
use crate::AppState;

const MAX_JD_BYTES: usize = 8_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationPack {
    pub application_id: String,
    pub path: String,
    pub markdown: String,
}

fn json_body(text: &str) -> &str {
    let start = text.find('{');
    let end = text.rfind('}');
    match (start, end) {
        (Some(start), Some(end)) if end > start => &text[start..=end],
        _ => text.trim(),
    }
}

fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn section_of(markdown: &str, heading: &str) -> Option<String> {
    let start = markdown.find(heading)?;
    let rest = &markdown[start + heading.len()..];
    let end = rest.find("\n## ").unwrap_or(rest.len());
    let body = rest[..end].trim();
    if body.is_empty() {
        None
    } else {
        Some(body.to_string())
    }
}

fn first_paragraph(resume: &str) -> Option<String> {
    let lines: Vec<&str> = resume
        .lines()
        .map(str::trim)
        .skip_while(|line| line.is_empty() || line.starts_with('#') || line.starts_with("---"))
        .take_while(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join(" "))
    }
}

fn missing(command: &str) -> String {
    format!("_Not generated yet — run `{}`._", command)
}

/// Key skills are copied from the resume's own competencies section; the pack
/// never invents them from claim metadata.
fn core_competencies(resume: &str) -> Vec<String> {
    let mut skills: Vec<String> = Vec::new();
    let mut in_section = false;
    for line in resume.lines() {
        let trimmed = line.trim();
        if !in_section {
            if trimmed.starts_with('#') && trimmed.to_lowercase().contains("core competencies") {
                in_section = true;
            }
            continue;
        }
        if trimmed.starts_with('#') {
            break;
        }
        if !trimmed.starts_with('|') {
            continue;
        }
        if trimmed
            .chars()
            .all(|character| matches!(character, '|' | '-' | ':' | ' '))
        {
            continue;
        }
        for cell in trimmed.split('|') {
            let skill = cell.trim().trim_start_matches('•').trim();
            if !skill.is_empty() && !skills.iter().any(|existing| existing == skill) {
                skills.push(skill.to_string());
            }
        }
    }
    skills
}

/// Assemble the pack from artifacts that already exist. No timestamp: the same
/// artifact set must produce the same bytes.
pub(crate) fn assemble_pack(
    employer: &str,
    role: &str,
    claims: &[Claim],
    resume: Option<&str>,
    letter: Option<&str>,
    prep: Option<&str>,
    questions: Option<&[String]>,
    dm: Option<&str>,
) -> String {
    let mut md = format!("# Application pack — {} — {}\n\n", employer, role);

    md.push_str("## CV summary\n\n");
    md.push_str(
        &resume
            .and_then(first_paragraph)
            .unwrap_or_else(|| missing("/apply")),
    );
    md.push_str("\n\n## Key skills\n\n");
    let skills: Vec<String> = resume.map(core_competencies).unwrap_or_default();
    if skills.is_empty() {
        md.push_str(&missing("/apply"));
    } else {
        for skill in &skills {
            md.push_str(&format!("- {}\n", skill));
        }
    }

    md.push_str("\n## Resume\n\n");
    md.push_str(resume.map(str::trim).filter(|text| !text.is_empty()).unwrap_or(&missing("/apply")));
    md.push_str("\n\n## Cover letter\n\n");
    md.push_str(letter.map(str::trim).filter(|text| !text.is_empty()).unwrap_or(&missing("/apply")));

    md.push_str("\n\n## Question bank\n\n");
    md.push_str(
        &prep
            .and_then(|text| section_of(text, "## Question bank"))
            .unwrap_or_else(|| missing("/questions")),
    );

    md.push_str("\n\n## Questions for the interviewer\n\n");
    match questions.filter(|items| !items.is_empty()) {
        Some(items) => {
            for (index, question) in items.iter().enumerate() {
                md.push_str(&format!("{}. {}\n", index + 1, question));
            }
        }
        None => md.push_str(&missing("/pack")),
    }

    md.push_str("\n## Recruiter DM\n\n");
    md.push_str(dm.map(str::trim).filter(|text| !text.is_empty()).unwrap_or(
        "_Not generated yet — no claim-backed message passed review._",
    ));

    md.push_str("\n\n## Follow-up email\n\n");
    md.push_str(&missing("/followup"));

    md
}

fn numeric_tokens(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_ascii_digit())
        .filter(|token| !token.is_empty())
        .map(|token| token.trim_start_matches('0').to_string())
        .filter(|token| !token.is_empty())
        .collect()
}

fn unsupported_numbers(text: &str, claims: &[Claim]) -> Vec<String> {
    let allowed: std::collections::HashSet<String> = claims
        .iter()
        .flat_map(|claim| {
            numeric_tokens(&claim.confirmed_wording)
                .into_iter()
                .chain(numeric_tokens(&claim.statement))
        })
        .collect();
    let mut unsupported: Vec<String> = numeric_tokens(text)
        .into_iter()
        .filter(|token| !allowed.contains(token))
        .collect();
    unsupported.sort();
    unsupported.dedup();
    unsupported
}

/// A recruiter DM must be short, claim-backed and free of placeholders.
pub(crate) fn dm_problem(dm: &str, claims: &[Claim]) -> Option<String> {
    let dm = dm.trim();
    if dm.is_empty() {
        return Some("the message is empty".to_string());
    }
    if dm.contains('[') || dm.contains(']') {
        return Some("the message contains a placeholder bracket".to_string());
    }
    let unsupported = unsupported_numbers(dm, claims);
    if !unsupported.is_empty() {
        return Some(format!(
            "the message states number(s) no claim supports: {}",
            unsupported.join(", ")
        ));
    }
    let metrics = verify_metrics(dm, claims);
    if !metrics.unverified.is_empty() {
        return Some(format!(
            "the message states unsupported metric(s): {}",
            metrics.unverified.join(", ")
        ));
    }
    if cited_claim_ids(dm, claims).is_empty() {
        return Some("the message cites no confirmed claim".to_string());
    }
    None
}

/// Each question must actually refer to something the posting names.
pub(crate) fn questions_problem(questions: &[String], jd_text: &str, claims: &[Claim]) -> Option<String> {
    if questions.is_empty() {
        return Some("no questions were generated".to_string());
    }
    let jd_words = keywords::significant_words(jd_text);
    for question in questions {
        let words = keywords::significant_words(question);
        if !words.iter().any(|word| jd_words.contains(word)) {
            return Some(format!(
                "the question does not refer to the posting: {}",
                truncate(question, 80)
            ));
        }
        let unsupported = unsupported_numbers(question, claims);
        if !unsupported.is_empty() {
            return Some(format!(
                "the question states number(s) no claim supports: {}",
                unsupported.join(", ")
            ));
        }
    }
    None
}

/// Deterministic, posting-grounded questions used when the model's attempt fails.
pub(crate) fn fallback_questions(jd_text: &str) -> Vec<String> {
    keywords::extract_phrases(jd_text)
        .into_iter()
        .filter(|term| term.contains(' '))
        .take(3)
        .map(|term| format!("How does the team measure success with {}?", term))
        .collect()
}

fn app_dir(application_id: &str) -> Result<std::path::PathBuf, String> {
    let docs = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    Ok(docs
        .join("CareerOS")
        .join("applications")
        .join(application_id))
}

fn read_document(application_id: &str, name: &str) -> Option<String> {
    let text = fs::read_to_string(app_dir(application_id).ok()?.join(name)).ok()?;
    (!text.trim().is_empty()).then_some(text)
}

fn latest_application(conn: &rusqlite::Connection) -> Result<(String, String, String), String> {
    conn.query_row(
        "SELECT id, employer, role FROM application_records ORDER BY date DESC, id DESC LIMIT 1",
        [],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        },
    )
    .map_err(|_| "No applications yet. Run /apply first.".to_string())
}

#[derive(Debug, Default, Deserialize)]
struct PackDraft {
    #[serde(default)]
    questions: Vec<String>,
    #[serde(default)]
    dm: String,
}

fn pack_prompt(role: &str, jd: &str, claims: &[Claim]) -> String {
    let mut prompt = format!("## Role\n{}\n\n## Posting\n{}\n\n## Confirmed claims\n", role, truncate(jd, MAX_JD_BYTES));
    for claim in claims {
        prompt.push_str(&format!("- [{}] {}\n", claim.id, claim.confirmed_wording));
    }
    prompt.push_str(
        "\nReturn only JSON: {\"questions\":[\"...\"],\"dm\":\"...\"}. \
Write three questions to ask the interviewer about priorities the posting states; do not invent company facts, culture or team size. \
Write a short recruiter DM (under 80 words) that names one or two confirmed claims and the role. No placeholder brackets, no numbers absent from the claims.",
    );
    prompt
}

/// Assemble the pack for the most recent application, or a named one.
#[tauri::command]
pub async fn generate_application_pack(
    application_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<ApplicationPack, String> {
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
        return Err("No confirmed claims. Complete Setup before building a pack.".to_string());
    }

    let (application_id, employer, role) = match application_id {
        Some(id) => {
            let (employer, role) = conn
                .query_row(
                    "SELECT employer, role FROM application_records WHERE id = ?1",
                    [id.as_str()],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .map_err(|_| format!("Application not found: {}", id))?;
            (id, employer, role)
        }
        None => latest_application(&conn)?,
    };

    let resume = read_document(&application_id, "resume.md");
    let letter = read_document(&application_id, "cover-letter.md");
    let prep = read_document(&application_id, "interview-prep.md");
    let jd = read_document(&application_id, "job-description.md").unwrap_or_default();
    if resume.is_none() && letter.is_none() {
        return Err("No generated materials for this application. Run /apply first.".to_string());
    }

    let config = crate::ollama::get_ollama_config(state.clone()).unwrap_or_default();
    let model = config
        .fast_model
        .clone()
        .or_else(|| config.deep_model.clone())
        .unwrap_or_else(|| "llama3.2:latest".to_string());

    let mut questions: Vec<String> = Vec::new();
    let mut dm: Option<String> = None;
    let mut messages = vec![
        OllamaChatMessage {
            role: "system".to_string(),
            content: "You write short application-pack content from a confirmed claims ledger. \
Never invent facts. Return JSON only."
                .to_string(),
        },
        OllamaChatMessage {
            role: "user".to_string(),
            content: pack_prompt(&role, &jd, &claims),
        },
    ];

    for _ in 0..2 {
        let Ok(response) = chat_with_ollama_async(
            model.clone(),
            messages.clone(),
            Some(0.3),
            Some(1400),
            Some("json".to_string()),
            config.num_ctx,
        )
        .await
        else {
            break;
        };
        let Ok(mut draft) = serde_json::from_str::<PackDraft>(json_body(&response.message.content)) else {
            break;
        };
        if questions_problem(&draft.questions, &jd, &claims).is_none() && !draft.questions.is_empty() {
            questions = std::mem::take(&mut draft.questions);
        }
        if dm_problem(&draft.dm, &claims).is_none() {
            dm = Some(draft.dm.trim().to_string());
        }
        if !questions.is_empty() && dm.is_some() {
            break;
        }
        messages.push(OllamaChatMessage {
            role: "user".to_string(),
            content: "That response failed review. Return corrected JSON; questions must refer to posting terms and the DM must cite a confirmed claim with no brackets.".to_string(),
        });
    }

    if questions.is_empty() {
        questions = fallback_questions(&jd);
    }

    let markdown = assemble_pack(
        &employer,
        &role,
        &claims,
        resume.as_deref(),
        letter.as_deref(),
        prep.as_deref(),
        Some(&questions),
        dm.as_deref(),
    );

    let dir = app_dir(&application_id)?;
    let path = dir.join("application-pack.md");
    fs::write(&path, &markdown)
        .map_err(|error| format!("Failed to write application-pack.md: {}", error))?;

    Ok(ApplicationPack {
        application_id,
        path: path.to_string_lossy().into_owned(),
        markdown,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        assemble_pack, dm_problem, fallback_questions, questions_problem,
    };
    use crate::claims::Claim;

    fn claim(id: &str, wording: &str) -> Claim {
        Claim {
            id: id.to_string(),
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

    #[test]
    fn missing_artifacts_are_named_with_their_command() {
        let pack = assemble_pack(
            "Aurora",
            "Learning Lead",
            &[claim("claim-1", "Cut reporting costs by 12%")],
            None,
            None,
            None,
            None,
            None,
        );
        assert!(pack.contains("run `/apply`"));
        assert!(pack.contains("run `/questions`"));
        assert!(pack.contains("run `/followup`"));
        assert!(!pack.contains("## Resume\n\n\n"));
    }

    #[test]
    fn key_skills_are_copied_from_the_resume_not_claim_metadata() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        let resume = "## #Ada\n\n#### Core Competencies\n\n| • Instructional Design | • Facilitation |\n| --- | --- |\n\n#### Technical Skills\n\n**Rust**\n";
        let pack = assemble_pack(
            "Aurora",
            "Learning Lead",
            &claims,
            Some(resume),
            None,
            None,
            None,
            None,
        );
        assert!(pack.contains("- Instructional Design\n- Facilitation\n"), "{}", pack);
        assert!(!pack.contains("- project"));
        assert!(!pack.contains("- Rust"));
    }

    #[test]
    fn the_pack_is_deterministic() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        let resume = "# Ada\n\nLearning lead with eight years in enterprise training.";
        let letter = "Dear Aurora team, I cut reporting costs by 12%.";
        let prep = "## Question bank\n\n- Tell me about a time you led.\n\n## STAR bank\n";
        let questions = vec!["How does the team measure success with instructional design?".to_string()];
        let first = assemble_pack(
            "Aurora",
            "Learning Lead",
            &claims,
            Some(resume),
            Some(letter),
            Some(prep),
            Some(&questions),
            Some("Hello, I cut reporting costs by 12%."),
        );
        let second = assemble_pack(
            "Aurora",
            "Learning Lead",
            &claims,
            Some(resume),
            Some(letter),
            Some(prep),
            Some(&questions),
            Some("Hello, I cut reporting costs by 12%."),
        );
        assert_eq!(first, second);
        assert!(first.contains("Tell me about a time you led."));
        assert!(first.contains("How does the team measure success"));
    }

    #[test]
    fn assembling_does_not_change_the_artifacts() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        let resume = "# Ada\n\nLearning lead with eight years in enterprise training.";
        let letter = "Dear Aurora team, I cut reporting costs by 12%.";
        let before = serde_json::to_string(&crate::compliance::digest_text("resume.md", resume)).unwrap();
        let letter_before =
            serde_json::to_string(&crate::compliance::digest_text("cover-letter.md", letter)).unwrap();

        let _ = assemble_pack(
            "Aurora",
            "Learning Lead",
            &claims,
            Some(resume),
            Some(letter),
            None,
            None,
            None,
        );

        assert_eq!(
            before,
            serde_json::to_string(&crate::compliance::digest_text("resume.md", resume)).unwrap()
        );
        assert_eq!(
            letter_before,
            serde_json::to_string(&crate::compliance::digest_text("cover-letter.md", letter)).unwrap()
        );
    }

    #[test]
    fn the_recruiter_dm_rejects_placeholders_and_unknown_numbers() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        assert!(dm_problem("Hi [Name], I cut reporting costs by 12%.", &claims).is_some());
        assert!(dm_problem("Hi Sam, I cut reporting costs by 45%.", &claims).is_some());
        assert!(dm_problem("Hi Sam, I would love to talk about the role.", &claims).is_some());
        assert!(dm_problem("Hi Sam, I cut reporting costs by 12% at Northwind.", &claims).is_none());
    }

    #[test]
    fn interviewer_questions_must_refer_to_the_posting() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        let jd = "Lead instructional design programs and own the learning technology roadmap.";
        assert!(
            questions_problem(&["What is the team culture like?".to_string()], jd, &claims).is_some()
        );
        assert!(questions_problem(
            &["How is the learning technology roadmap prioritised?".to_string()],
            jd,
            &claims
        )
        .is_none());
    }

    #[test]
    fn fallback_questions_are_grounded_in_posting_terms() {
        let jd = "Lead instructional design programs and own the learning technology roadmap.";
        let questions = fallback_questions(jd);
        assert!(!questions.is_empty());
        assert!(questions_problem(&questions, jd, &[]).is_none());
    }
}
