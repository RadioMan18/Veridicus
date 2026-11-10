# Veridicus AI system card

**System:** Veridicus Desktop, version 0.1.0
**Document status:** Phase 0–P2 compliance evidence. This is not a certification.
**Last reviewed:** 2026-11-07

## Purpose

Veridicus helps a job seeker tailor a resume and cover letter to a specific posting using only
facts the user has confirmed. It records what it generated and what the honesty gate decided.

## Intended use

- One user, on their own computer, preparing applications for their own career.
- The user remains the author and the decision-maker. Generated text is a draft for review.
- The user must confirm claims before they can be used.

## Non-goals and prohibited uses

- Not a truth oracle for the job posting, the employer, or the user's history.
- Not for fabricating experience, metrics, credentials, or scope.
- Not for submitting without human review.
- Not for another person's resume or data without their knowledge.

## How it works

1. The user confirms factual claims in an encrypted local ledger.
2. A local Ollama model drafts the documents from those claims, the base resume (structure and
   phrasing only), and the posting.
3. Deterministic checks run on the draft: never-claim subjects, exact-wording rules, numeric
   metrics with qualifiers, scope escalation, required-gap naming, employer/role naming, and
   resume-bullet grounding.
4. An advisory model pass looks for subtler scope inflation. Its findings are non-blocking and
   must cite a real claim and quote the draft.
5. The user reviews both documents and approves. Blocking findings stop the commit unless the user
   records a written override.
6. An append-only audit record is written at generation and at commit.

## What is verified

- Every number appears in a confirmed claim, with its qualifier intact.
- No never-claim subject appears.
- Approved exact wording is not altered.
- Resume bullets trace to a confirmed claim.
- Must-have and gate gaps are named in the cover letter or accepted by the user.
- The letter names the employer and the role.

## What is not verified

- The letter's free prose is grounded at generation time, not re-grounded sentence by sentence at
  commit. A manually edited letter is outside that check.
- The advisory model can miss subtle inflation and can produce false positives. It is not the
  authority; the deterministic gate is.
- The posting's accuracy, the employer's intentions, and the user's evidence quality are not
  assessed.
- Legal, immigration, security-clearance, and eligibility statements are the user's responsibility.

## Human oversight

- First launch requires acknowledgement of this disclosure and intended use; the acknowledgement is
  stored in the workspace and `compliance/disclosures.jsonl`.
- Approval is a deliberate action in the review dialog. The user must also acknowledge the
  per-application AI-assisted disclosure; a `disclosure_acknowledged` audit event is written, and
  commit is refused without an acknowledgement that follows the latest generation.
- A blocking finding requires a written reason; the reason, timestamp, and finding summary are
  stored in `guardrail-override.md` and the audit record.
- The user can report a structured incident with a category and note; the row is appended to
  `reported-problems.jsonl` and can be exported as CSV.

## Data and retention

- Claims, rules, gaps, and application records live in a SQLCipher-encrypted SQLite database.
- Documents and audit records live under `Documents/Veridicus/applications/<id>/`.
- Inference is local through Ollama; no document text is sent to a hosted model by Veridicus.
- Disclosure acknowledgements and 90-day honesty reviews live in `compliance/`.
- The Git repository in the workspace keeps a versioned snapshot of approved materials.
- Retention and deletion are controlled by the user; there is no automatic expiry.

## Known limitations

- Small local models vary in quality; the same prompt can produce different drafts.
- Model tags such as `:latest` are mutable. The audit record stores the digest observed at
  generation and at commit so a change is visible.
- The deterministic scope lexicon is finite; the advisory pass exists to widen coverage, not to
  guarantee it.
- The employer-consistency wording check is not active until an approved-wording workflow exists.
  It is not reported as coverage.
- Claims created before provenance existed default to `evidence_type = unspecified` and appear as weak
  provenance until re-confirmed.
- Instruction-like posting text is detected heuristically; a missed attempt is possible, which is why
  the posting is always delimited as data and never treated as instructions.
- The honesty review record is local and unsigned; it demonstrates a cadence, not independent audit.

## Governance note

This card describes the application. ISO/IEC 42001 certification applies to an organization's AI
management system, including policy, roles, impact assessment, internal audit, and management
review. Draft artifacts now exist in the repository:

- [AI-POLICY.md](AI-POLICY.md) — policy and prohibited uses.
- [AI-RACI.md](AI-RACI.md) — roles, decision rights and independence limits.
- [CONCERNS-CHANNEL.md](CONCERNS-CHANNEL.md) — reporting, triage and appeal.
- [AI-IMPACT-ASSESSMENT.md](AI-IMPACT-ASSESSMENT.md) — affected groups, impacts and change triggers.
- [HONESTY-OBJECTIVES.md](HONESTY-OBJECTIVES.md) — measurable objectives.
- [INTERNAL-AUDIT.md](INTERNAL-AUDIT.md) and [MANAGEMENT-REVIEW.md](MANAGEMENT-REVIEW.md) — evaluation
  and improvement procedures.
- [STATEMENT-OF-APPLICABILITY.md](STATEMENT-OF-APPLICABILITY.md) — all 38 Annex A controls and their
  applicability.
- [COMPLIANCE-EVIDENCE.md](COMPLIANCE-EVIDENCE.md) — control-to-artifact index.

These are drafts until the operating organization names role holders, fills the concerns contact,
and signs the approval blocks. No audit or management review has been performed. This card and those
artifacts do not create a certified management system.
