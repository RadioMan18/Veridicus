# AI system impact assessment — Veridicus honesty guardrails

**Artifact ID:** C-08
**Version:** 2.0 (supersedes the initial 2026-11-07 assessment)
**Framework:** ISO/IEC 42001:2023 clauses 6.1.4 / 8.4 and Annex A.5.2–A.5.5; method informed by
ISO/IEC 42005:2025.
**Scope:** the honesty and guardrail behavior of Veridicus Desktop. Privacy, security, fairness and
accessibility are considered where they affect truthful output; they are not fully assessed here.
**Status:** working assessment for the demonstrator. Owner sign-off: **pending**.

## 1. System and context

Veridicus generates job-application documents from a user-confirmed claims ledger using a local
language model. The user reviews and approves the result. The application does not submit
applications, rank candidates or make employment decisions.

The system is used by one job seeker on their own computer. Its output can still reach an employer,
a recruiter and people named in the user's claims. That reach is the reason the assessment treats a
fluent but false draft as a real harm, not a cosmetic defect.

## 2. Method

This assessment follows the ISO/IEC 42005 sequence in a form proportionate to a single-user
demonstrator:

1. **Context and scope:** define the system, its users, the decision it supports and its boundaries.
2. **Affected parties:** identify who can be affected and how.
3. **Impact identification:** identify harms, including misrepresentation, automation bias,
   over-reliance on advisory findings and loss of recourse.
4. **Evaluation:** rate severity and likelihood before and after treatment, and record residual risk.
5. **Treatment:** specify controls, owners and evidence.
6. **Monitoring and review:** define metrics, triggers and the review cadence.

The assessment is qualitative. Severity reflects harm to a job seeker, an employer or a named third
party; likelihood reflects the current design, not a statistical estimate. No claim of quantitative
risk measurement is made.

## 3. Affected parties

| Party | How affected | Vulnerability |
|---|---|---|
| **Job seeker (the user)** | A false or inflated application can damage reputation, waste an opportunity or trigger a hiring consequence. A false block can prevent a truthful application. | Bears the outcome; may over-trust fluent output; may have limited time to review. |
| **Job seekers as a group** | If the tool normalizes inflated claims, it can disadvantage honest applicants and erode trust in applications generally. | Diffuse but real; harder to observe in a single use. |
| **Employers and recruiters** | Receive documents that may overstate scope, metrics, credentials or eligibility; spend time verifying or rejecting. | Rely on the document at face value. |
| **People named in claims** | A colleague, manager or client may be described inaccurately or associated with an inflated result. | May not know the document exists; limited recourse. |
| **People with protected characteristics** | Inference or wording could indirectly signal or rely on a protected trait, even though the system is not a screening tool. | Indirect and context-dependent; not fully assessed. |
| **Operator / maintainer** | A missed false claim or a guardrail defect creates legal, reputational and ethical exposure. | Accountable for the system's behavior. |

## 4. Impact evaluation

| ID | Impact | How it could occur | Severity | Likelihood | Treatment | Residual |
|---|---|---|---|---|---|---|
| **I-1** | **Misrepresentation** | Model invents a metric, scope, credential or employer detail; a qualifier is dropped; free prose drifts beyond the cited claim. | High | Medium | Ledger-only generation; deterministic gate; qualifier-aware metrics; exact-wording checks; commit-time re-check; override requires a written reason. | Low–medium. Paraphrase and novel prose can still evade pattern checks; the gate is not a truth oracle. |
| **I-2** | **Automation bias** | User trusts a fluent draft, skips the findings, or approves because the tool looks confident. | Medium | Medium | Review gate; findings beside documents; explicit AI-assisted disclosure; per-application acknowledgement; no auto-submit. | Medium. A determined user can click through; fluency can still persuade. |
| **I-3** | **Over-reliance on advisory findings** | User treats the advisory scope pass as proof of honesty, or assumes a clean report means the document is true. | Medium | Medium | Advisory findings are non-blocking, must cite a real claim and a quote, and are labeled advisory; system card states the deterministic gate is authoritative. | Low–medium. The distinction is only effective if the user reads it. |
| **I-4** | **Loss of recourse** | A false statement is submitted; the user cannot correct the record, the employer has no route to challenge it, and the local incident log is not an independent appeal. | High | Medium | Human approval before submission; incident reporting and export; concerns channel; appeal to an independent reviewer; disclosure. | Medium–high. The app cannot retract a sent application; external recourse depends on the employer and the operator. |
| **I-5** | **Omission of a material gap** | The letter fails to name a must-have gap; the user appears to claim a fit they do not have. | Medium | Medium | Required gaps injected into the letter prompt; naming check; block on omission; per-application gap selection remains partly manual. | Low. |
| **I-6** | **False block / over-blocking** | A true statement is rejected; the user overrides routinely or abandons a valid application. | Medium | Medium | Advisory versus blocking distinction; written override; false-block incident category; evaluation fixtures; 90-day review. | Medium. Override fatigue is a monitoring signal, not a solved problem. |
| **I-7** | **Privacy of claims** | Sensitive claim content leaves the machine through a hosted model or a shared workspace. | High | Low | Local Ollama; encrypted SQLCipher database; no hosted inference by Veridicus; local audit records. | Low, contingent on the user's machine and model configuration. |
| **I-8** | **Instruction injection** | A posting contains instructions that try to make the model ignore the ledger or the guardrails. | Medium | Medium | Posting delimited as data; instruction-like lines detected; finding and audit note recorded; deterministic gate still applies. | Low–medium. Detection is heuristic; a missed attempt is possible. |
| **I-9** | **Discriminatory or protected-trait inference** | Generated wording or a claim references a protected characteristic, or the tool is repurposed for screening. | High | Low | Intended-use restriction; no screening features; prohibited-use policy; review and incident channel. | Medium. The system is not a fairness engine; this needs a separate assessment if use changes. |
| **I-10** | **False reassurance from a clean report** | A clean deterministic report is read as “the document is true” rather than “the checked patterns passed.” | Medium | Medium | System card limits; advisory/deterministic distinction; disclosure; audit record shows what was checked. | Low if the limits are read. |

## 5. Treatment and evidence

| Treatment | Evidence |
|---|---|
| Claims ledger is the only factual authority | `claims.rs`; prompt contract; guardrail tests; `claims_used` records cited claims only |
| Deterministic gate blocks unsupported statements | `guardrails.rs`; commit gate; `eval/guardrails.json`; 10 adversarial fixtures |
| Human approval and disclosure | Review dialog; `disclosure_acknowledged` audit event; first-run disclosure; no auto-commit |
| Override is visible and recorded | `guardrail-override.md`; override audit event; written reason required |
| Provenance and weak-provenance advisory | `evidence_type`, reference, SHA-256 fingerprint, `verified_by`, `last_confirmed_at`; `verify_claim_evidence` |
| Incident reporting and export | `report_output_problem`; `reported-problems.jsonl`; CSV export |
| Monitoring and 90-day review | `/honesty-stats`; Settings honesty panel; `compliance/reviews.jsonl` |
| Untrusted-input boundary | JD delimited; instruction-like lines flagged; injection fixture |
| Governance and recourse | AI-POLICY; AI-RACI; CONCERNS-CHANNEL; HONESTY-OBJECTIVES; INTERNAL-AUDIT; MANAGEMENT-REVIEW |

## 6. Review triggers for model, rule and template changes

A change is **material** if it can alter which statements are generated, which statements are blocked,
or how a user understands the output. Material changes require this assessment to be reviewed and
updated before release, plus a regression run and an audit-record note.

| Trigger | Examples | Required action | Owner | Evidence |
|---|---|---|---|---|
| **Model change** | New model family, tag/digest change, different quantization, different inference runtime, changed context window | Re-run the evaluation suite; compare block/override behavior; update model identity and limitations in the system card; record the digest; review I-1, I-2 and I-8 | SO | Model digest; test record; system-card revision |
| **Rule change** | New never-claim rule, changed scope lexicon, changed threshold, removed check, new finding severity | Classify as material if it changes a block; run before/after fixtures; update the rule hash and rule documentation; review I-1 and I-6 | SO | Rule diff; test output; audit note |
| **Template change** | Changed resume or cover-letter prompt, changed required-gap wording, changed output structure | Re-run generation fixtures; check claim grounding and gap naming; update template version; review I-1, I-2 and I-5 | SO | Template version; test output; sample document |
| **Threshold change** | Grounding window, metric tolerance, advisory confidence, context limit | Treat as material; run the adversarial fixtures; record the rationale and the expected behavior change | SO | Change record; test output |
| **Scope or distribution change** | Submission capability, hosted inference, another person's data, employer-facing use, multi-user | Stop and perform a new impact assessment before release; update the SoA and policy | AO | New assessment; SoA revision |
| **Serious incident** | Confirmed false claim in a submitted document, exposed data, repeated false blocks, discriminatory output | Contain; investigate; update this assessment; open corrective action; consider management review | AO/SO | Incident record; CAPA; updated assessment |
| **Legal or regulatory change** | New disclosure, data-protection, employment or AI obligation | Review policy, disclosure, retention and this assessment | AO | Legal review note; policy revision |

**Non-material changes** (formatting, internal refactor with no behavior change, documentation typo)
still require the normal tests, but do not require a new impact assessment. The change record should
state why the change is non-material.

## 7. Human oversight and recourse

- The user approves every application; there is no submission path.
- A blocking finding requires a written override, recorded with its findings and timestamp.
- The user can report a structured incident and export the log.
- The concerns channel accepts reports from users, employers and named third parties, with
  confidentiality, non-retaliation and an appeal to an independent reviewer.
- The app cannot retract a submitted document. Recourse after submission depends on the employer,
  the operator's incident process and applicable law; this limitation is stated rather than hidden.

## 8. Monitoring

The assessment is monitored through the objectives in
[HONESTY-OBJECTIVES.md](HONESTY-OBJECTIVES.md): confirmed ungrounded statements, disclosure
completeness, blocking-finding resolution, provenance completeness, incident timeliness, false-block
rate, guardrail pass rate, review cadence, advisory-only blocks and change control.

The 90-day honesty review is the operational check. The annual management review is the strategic
check. Neither substitutes for an independent audit.

## 9. Open items

- Claims created before P1 default to `evidence_type = unspecified`; they remain weak provenance until
  re-confirmed.
- Rule changes have no history table; the rule hash detects change but not the prior text.
- The advisory model pass and the disclosure/incident/review flow have not been exercised against a
  live Ollama instance in automated tests.
- The fairness and accessibility impacts (I-9) are identified but not fully assessed; a separate
  assessment is required if the system is used beyond a single job seeker.
- The governance artifacts are drafts; role appointments, signatures, the internal audit and the
  management review are organizational actions still outstanding.
- The local audit record is unsigned and can be altered by someone with file access; Git history makes
  casual alteration visible but is not tamper-proof.

## 10. Sign-off

This assessment is a working document. It must be reviewed and signed by the operating organization's
Accountable Owner before it is presented as compliance evidence.

| Role | Name | Signature | Date |
|---|---|---|---|
| Accountable Owner | | | |
| AI System Owner | | | |
| Independent Reviewer (challenge) | | | |
