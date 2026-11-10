# Guardrail method

**Scope:** how Veridicus decides whether a generated document is allowed to be committed.
**Code of record:** `src-tauri/src/guardrails.rs`, `fact_check.rs`, `scope_review.rs`.

## Authority order

1. **Deterministic checks** decide. They can block a commit.
2. **Model output** is a proposal. The advisory scope pass can warn, never block.
3. **The user** can approve or record a written override. The override is evidence, not a pass.

## Inputs

- Confirmed claims: id, confirmed wording, scope, evidence source, confirmation time.
- Active rules: `never_claim`, `ask_first`, `exact_wording`, `presentation`.
- Required gaps: must-have and gate requirements not supported by a claim.
- Employer and role from the posting.
- The rendered `resume.md` and `cover-letter.md`.

## Deterministic checks

### Never-claim and ask-first

Case-insensitive substring match of the rule subject against the draft. A match blocks. The same
subjects are passed into the fact-check pass, so the rule is evaluated twice.

### Exact wording

If the subject appears, the approved `rule_text` must also appear verbatim. Altering the approved
phrasing blocks.

### Metrics and qualifiers

Every percentage, currency figure, duration, multiplier, fraction, and count is extracted. Each
must match a confirmed claim on **value plus qualifier**. `up to 50%` cannot become `50%`, and
`50%` cannot become `up to 50%`.

### Scope

The deterministic pass flags known escalation terms (`enterprise-wide`, `company-wide`,
`org-wide`, `firm-wide`, `across the company/organization`) and ownership verbs (`led`, `own`)
when the matched claim describes contribution (`supported`, `helped`, `contributed`, `assisted`,
`participated`).

### Grounding

- A sentence or bullet is grounded when it shares a four-word verbatim window with a confirmed
  claim, or when at least 60% of its content words appear in the claim.
- Resume bullets shorter than 20 characters are ignored.
- `## Education`, `## Certifications`, and `## Certificates` sections are skipped.
- An ungrounded resume bullet blocks.

### Gaps

For a cover letter, every must-have or gate gap not marked `accepted_by_user` must be named in one
sentence. A full phrase match or at least 50% of the requirement's content words in one sentence
counts as named.

### Employer and role

The letter must contain the employer name and the role title, unless either is missing or
`Unknown`.

### Claim provenance

Each confirmed claim carries an evidence type, a reference, a SHA-256 provenance fingerprint, the
verifier's name and the confirmation time. A cited claim with an unspecified type, a reference shorter
than four characters, no fingerprint or no verifier produces a non-blocking
`weak_claim_provenance` finding in the review and the audit record. The fingerprint is a tamper-evidence
check over the provenance fields, not a hash of an external document.

### Untrusted job description

The posting is delimited with `<<<JOB_DESCRIPTION` / `JOB_DESCRIPTION>>>` and labelled as data, never
instructions. Instruction-like lines (`ignore previous`, `system prompt`, `you are now`, `jailbreak`,
and similar) produce a warning in the prompt, a non-blocking `untrusted_input_instruction` finding,
and a note in the generation audit event. Detection is heuristic; the delimiter is the primary control.

## Advisory scope review

- Only claims that appear in the document are sent to the model.
- The model must return a claim id, the exact sentence, and a reason.
- Observations citing an unknown claim or a sentence not present in the document are discarded.
- At most five findings are kept.
- Findings are `scope_review_advisory`, non-blocking. If the model is unavailable, the report says
  `scope_review_unavailable` instead of claiming the review happened.

## Commit gate

The generation pipeline never commits; it stops at the review gate. `commit_application` requires an
AI-assisted disclosure acknowledgement written after the latest generation. It then re-reads the
rendered files, re-runs the deterministic evaluation with the current rules and claims, and writes
`guardrail-report-commit.json`. Blocking findings stop the commit. A written override is stored in
`guardrail-override.md`, appended to the application notes, and recorded as an `override` audit event.

## Audit record

`applications/<id>/audit/events.jsonl`, append-only:

- event id, UTC timestamp, app version
- application id and event type (`generated`, `disclosure_acknowledged`, `approved`, `override`)
- model name, generation digest, commit-time digest
- template version, rule ids, rule-set SHA-256
- SHA-256 and byte size of each document
- blocking verdict, findings, advisory count
- override reason when present

## Known failure modes

- **Paraphrase drift:** a heavily paraphrased claim may not share a four-word window and may not
  reach 60% overlap. The generator is instructed to stay close to confirmed wording; a missed
  paraphrase is a false negative.
- **Lexicon gaps:** the deterministic scope list is finite. The advisory pass widens coverage but
  cannot guarantee it.
- **False positives:** a legitimate claim can be blocked when it resembles a never-claim subject,
  or when a bullet is genuinely new but unsupported. The override path exists for this.
- **Letter prose:** grounding is enforced at generation time. The commit gate checks metrics, scope,
  gaps, and naming, not every letter sentence.
- **Rules change between generation and commit:** the commit re-runs with current rules. The audit
  record stores both rule hashes when they differ, but does not preserve the old rule text.
- **Weak provenance:** legacy claims or hastily confirmed claims can carry an unspecified evidence
  type. The advisory names the missing field; it does not block, because the claim itself is already
  confirmed.
- **Injection false negatives:** detection covers common instruction patterns only. The delimiter and
  the system prompt's data-only rule remain the primary controls.

## Evaluation

`src-tauri/eval/guardrails.json` contains end-to-end fixtures for fabricated metrics, dropped
qualifiers, scope escalation, never-claim, omitted gaps, ungrounded bullets, employer mismatch, and
prompt injection. `cargo test` runs them through the assembled gate. Unit tests beside each check
cover the individual semantics.

## Change control

- Template changes bump `TEMPLATE_VERSION`; existing workspace templates are upgraded with a
  `.bak` copy.
- Rule changes alter the rule-set hash recorded in the next audit event. There is no rule-change
  history table yet; this is a known gap for audit reconstruction.
