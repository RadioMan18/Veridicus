# Auditor evidence request — first internal audit

**Audit ID:** IA-________-01
**Request ID:** ER-________-01
**Status:** **Template; not issued.** Complete and issue only when the audit is scheduled.
**Issued by:** ______________________________
**Issued to:** ______________________________
**Issue date:** ____________________
**Response due:** ____________________
**Audit period:** ____________________ to ____________________
**Governing workpaper:** [WP-01 §6](WP-01-PLANNING.md)
**Tracking sheet:** [EVIDENCE-MANIFEST.csv](EVIDENCE-MANIFEST.csv)

> This request asks for evidence. It does not ask the provider to select samples, interpret results,
> or reach an audit conclusion. Supplying an item does not make it accepted; acceptance is recorded in
> the manifest after review.

## 1. Request summary

The audit needs the signed adoption record, application and audit-event exports, claim provenance
data, incident records and test output. The five priority items are ER-01 to ER-05. The supporting
items are ER-06 to ER-12.

The provider supplies the **full population** for each sampled population. The auditor selects the
sample using the rules in WP-01 §5 and records the seed and selected IDs. Do not send only a
convenience sample.

## 2. Priority requests

### ER-01 — Signed adoption record

**Source:** `docs/AI-POLICY.md`, `docs/AI-RACI.md`, `docs/HONESTY-OBJECTIVES.md`,
`docs/STATEMENT-OF-APPLICABILITY.md`, `docs/OPERATOR-ADOPTION-CHECKLIST.md`.

**Provide:**

- The approval page of each of the four artifacts, showing the named approver, signature and date.
- The completed adoption checklist, including §4 role appointments, §5 concerns contact, §6
  consolidated approvals and §9 adoption decision.
- If signatures are electronic, the signature record or audit trail that shows who signed and when.

**Format:** PDF or scanned signed pages plus the completed checklist. If a source artifact is stored
only as Markdown, provide a rendered PDF or a signed copy and state how the signature was applied.

**Acceptance:** All four approvals are signed and dated; the checklist names the AO, SO, MO and IR; the
independence declaration is completed; the concerns contact is filled in; no required field is blank
except an explicitly justified `N/A`.

**If unsigned:** return the item as “not available — unsigned.” Do not substitute a draft or a verbal
approval. The audit will record the missing approval as a limitation or finding.

**Used in:** WP-01 §6; WP-02 §2.

### ER-02 — Application and audit-event exports

**Source:** Application records and `applications/<id>/audit/events.jsonl`.

**Provide first — full population index (CSV):**

```
application_id, generated_at_utc, approved_at_utc, gate_verdict,
blocking_finding_count, override_yes_no,
disclosure_ack_after_latest_generation, model_name, model_digest,
resume_sha256, cover_letter_sha256
```

Include every approved application in the audit period. Do not omit an application because it is
inconvenient or because it contains an override.

**Provide after selection — one folder per sampled application:**

- `resume.md` and `cover-letter.md`;
- `audit/events.jsonl`;
- `guardrail-report.json` and `guardrail-report-commit.json`;
- `guardrail-override.md`, if an override exists;
- the incident record, if the application has one.

**Format:** CSV index plus a ZIP or folder per sampled application. Preserve file names and relative
paths. Include a SHA-256 for each file in the manifest.

**Acceptance:** IDs in the index match the folders; timestamps are UTC; gate verdict and override flag
are present; the audit events include the generation, disclosure, approval/override and model digest;
the commit report is separate from the apply report.

**Used in:** WP-03 §2, §4; WP-04 §3.

### ER-03 — Claim and provenance population

**Source:** Claims ledger; `verify_claim_evidence` output.

**Provide — full cited-claim list (CSV):**

```
claim_id, application_id, cited_in_document, evidence_type,
evidence_reference, provenance_sha256, verified_by,
last_confirmed_at, weak_provenance_flag
```

Include every claim cited by the approved applications in the period. Also provide the output of
`verify_claim_evidence` for the same claims, including mismatches and missing values.

**Format:** CSV plus the verification output. If a claim is legacy and has `evidence_type =
unspecified`, keep it in the population and mark it; do not remove it.

**Acceptance:** The auditor can select at least 10 cited claims, including a weak-provenance claim if
one exists. Fingerprints match the recomputation, or mismatches are clearly reported. The provider has
not preselected a favourable sample.

**Used in:** WP-03 §3.

### ER-04 — Incident and concerns log

**Source:** `reported-problems.jsonl`; CSV export; concerns register.

**Provide:**

- The full incident CSV for the period.
- All critical and high incident records, including the category, severity, acknowledgement,
  investigation, root cause, corrective action, owner, due date, closure and appeal.
- The test record for the external concerns route from the adoption checklist §5.
- A statement of whether any concern was received outside the in-app channel.

**Format:** CSV plus the detail records. Redact unnecessary personal data; keep incident IDs and
timestamps.

**Acceptance:** Every critical/high incident is present; categories and severity are recorded;
acknowledgement dates can be compared with the targets; corrective actions have owners and dates;
appeals are recorded; no incident was removed because it reflected poorly on the system.

**Used in:** WP-03 §5; WP-05 §2.

### ER-05 — Test output and reviewed snapshot

**Source:** Repository; test/build commands.

**Provide:**

- `git rev-parse HEAD`.
- `git status --porcelain` at the audit snapshot.
- Full output and exit status for:
  - `cd src-tauri && cargo test --quiet`
  - `cd src-tauri && cargo build --quiet`
  - `npm run build`
- The test count, any ignored/skipped tests, and build warnings.
- If the working tree is dirty, an archive or content hash of the reviewed tree.

**Format:** Text or log files, with the command, working directory, timestamp and exit status. Do not
copy a prior report without rerunning the commands at the audit snapshot.

**Acceptance:** The output is from the reviewed snapshot, not a later or earlier commit; exit statuses
are visible; actual test counts are recorded; a dirty tree is identified and archived.

**Used in:** WP-04 §1–§2.

## 3. Supporting requests

| ID | Item | Source | Format | Acceptance | Workpaper |
|---|---|---|---|---|---|
| ER-06 | Signed impact assessment | `docs/AI-IMPACT-ASSESSMENT.md` | Signed PDF or signed page | Affected group, four primary risks, triggers and sign-off present | WP-02 §5 |
| ER-07 | Concerns contact and test record | Adoption checklist §5 | PDF/CSV plus test message | Contact monitored; test acknowledged; escalation and appeal stated | WP-03 §5 |
| ER-08 | Honesty reviews and metrics | `compliance/reviews.jsonl`; metrics summary | JSONL/CSV plus summary | Reviewer, date, note and denominators present; 90-day cadence visible | WP-03 §6 |
| ER-09 | Release and change log | Release record; change log | CSV/Markdown | Material changes have impact review, regression run and audit note | WP-03 §7; WP-04 §5 |
| ER-10 | Model identity and supplier limitations | System card; audit events | Markdown/CSV | Name, digest, runtime and known limitations recorded; no hosted inference | WP-03 §8 |
| ER-11 | Code snapshot | Repository | ZIP or tree hash plus `git status` | Snapshot matches the commit/dirty state recorded in ER-05 | WP-04 §1 |
| ER-12 | CAPA and prior management decisions | Organization's CAPA register; management review | CSV/Markdown | Owners, due dates, status and effectiveness checks present; prior decisions listed | WP-05 §4; WP-06 §5 |

## 4. File naming and manifest

Use this pattern so the receipt can be matched to the request:

```
ER-01_adoption-record_YYYYMMDD.pdf
ER-02_application-population_YYYYMMDD.csv
ER-02_app-<application-id>_YYYYMMDD.zip
ER-03_claims_YYYYMMDD.csv
ER-03_verify-output_YYYYMMDD.txt
ER-04_incidents_YYYYMMDD.csv
ER-05_test-output_YYYYMMDD.txt
ER-05_git-status_YYYYMMDD.txt
ER-06_impact-assessment_YYYYMMDD.pdf
ER-07_concerns-test_YYYYMMDD.pdf
ER-08_reviews_YYYYMMDD.jsonl
ER-08_metrics_YYYYMMDD.csv
ER-09_change-log_YYYYMMDD.csv
ER-10_model-identity_YYYYMMDD.md
ER-11_snapshot_YYYYMMDD.zip
ER-12_capa_YYYYMMDD.csv
```

For every file, complete [EVIDENCE-MANIFEST.csv](EVIDENCE-MANIFEST.csv): request ID, file name,
SHA-256, file count/pages, received date and status. Do not mark an item accepted until the auditor
has reviewed it.

## 5. Handling and confidentiality

- Transfer through the organization's access-controlled channel. Do not use a public link or a
  personal social-media account.
- Do not include credentials, API keys, session tokens, passwords or unnecessary personal data.
- Redact third-party names and sensitive details where they are not needed for the audit. Record what
  was redacted and why; do not alter the audit fields that prove what happened.
- Keep the evidence pack encrypted at rest and limit access to the auditor and the named roles.
- Record a SHA-256 at receipt. If a hash does not match the manifest, do not accept the item; request a
  fresh export.
- The local audit record is unsigned and can be altered by someone with file access. The audit treats
  Git history and hashes as corroboration, not as tamper-proof proof.
- Retain the pack for the organization's defined retention period. Do not delete it after the report.

## 6. Non-acceptance and follow-up

An item is not accepted when it is missing, altered, unsigned where a signature is required, copied
from a prior run without current output, or selected by the provider to avoid an unfavourable case.

| Situation | Response |
|---|---|
| Required signature missing | Record “not available — unsigned”; treat as a limitation/finding, not a pass. |
| Population incomplete | Request the missing IDs; record the gap. |
| Sample preselected by provider | Reject; request the full population and select in the audit. |
| Hash mismatch | Reject the file; request a fresh export from the source. |
| Sensitive data exposed | Stop transfer; contain; record an incident; do not copy the data into the report. |
| Item genuinely not applicable | Record the reason and the approver; do not write `N/A` without a basis. |

## 7. Request issue and response

### Issue record

| Field | Entry |
|---|---|
| Request issued by | |
| Date issued | |
| Delivery method | |
| Response due | |
| Auditor contact | |

### Provider response

| Field | Entry |
|---|---|
| Provider name / role | |
| Date | |
| Items included | |
| Items not available and why | |
| Redactions applied | |
| Signature | |

### Auditor receipt

| Field | Entry |
|---|---|
| Response received (date) | |
| Received by | |
| Manifest checked | yes / no |
| Hash mismatches | |
| Items accepted | |
| Items rejected / follow-up | |
| Findings opened | |

This package is a request and receipt record. It is not audit evidence until the items are received,
verified and used in the workpapers. It is not a certification.
