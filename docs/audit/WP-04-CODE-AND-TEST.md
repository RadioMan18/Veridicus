# WP-04 — Code, guardrail and test reproduction

**Audit ID:** IA-________-01
**Prepared by:** ______________________________  **Date:** ______________
**Reviewed by:** ______________________________  **Date:** ______________
**Status:** Not started

[Back to workpaper index](README.md)

## 1. Reviewed snapshot

Record exactly what was reviewed. A moving working tree is not an auditable snapshot.

| Field | Entry |
|---|---|
| Repository path | |
| Commit hash | |
| Branch | |
| `git status --porcelain` output attached? | yes / no |
| Working tree clean? | yes / no |
| If dirty: archive path / tree hash | |
| Audit date | |
| Model name and digest observed | |
| Template version(s) | |
| Rule-set hash | |

If the tree is dirty, archive the reviewed tree or record a content hash and audit that snapshot.
Do not claim a commit represents code that was not in the commit.

## 2. Reproduction commands

Run from the repository root unless stated otherwise. Capture full output and the exit status.

| # | Command | Last known result | Actual result | Exit | Notes |
|---|---|---|---|---|---|
| 1 | `cd src-tauri && cargo test --quiet` | 70 passed at 2026-11-07 | | | Record actual count; do not reuse the last known count |
| 2 | `cd src-tauri && cargo build --quiet` | exit 0 at 2026-11-07 | | | Record warnings |
| 3 | `npm run build` | exit 0 at 2026-11-07 | | | Record Vite output |
| 4 | `git status --porcelain` | working tree had uncommitted changes | | | Attach output |

**Test output notes**

```
______________________________________________________________________________
______________________________________________________________________________
```

- [ ] The test count and exit status were captured directly, not copied from a prior report.
- [ ] Any skipped or ignored required test is recorded as a finding or limitation.
- [ ] Build warnings are recorded and assessed for security or correctness impact.
- [ ] The frontend build used the project's existing package manager and lockfile.

## 3. Guardrail trace matrix

Trace each control to its implementation and its test/fixture. `Result` is blank until tested.

| Control / finding | Code path to inspect | Test or fixture | Expected behavior | Result | Finding |
|---|---|---|---|---|---|
| Never-claim subjects | `guardrails.rs`; rules loaded from DB | Unit test; evaluation fixture | Blocking finding when a never-claim subject appears | | |
| Ask-first subjects | `guardrails.rs` | Unit test | Blocking or user-input-required behavior as specified | | |
| Exact approved wording | `guardrails.rs`; approved-wording path | Unit test | Approved wording is not altered | | |
| Metric qualifier | `fact_check.rs`; `guardrails.rs` | Fixture for `up to 50%` vs `50%` | Value and qualifier must both match a confirmed claim | | |
| Scope escalation | `guardrails.rs`; `scope_review.rs` | Scope fixtures | Deterministic scope finding blocks where specified | | |
| Required-gap naming | `guardrails.rs`; `orchestrator.rs` | Gap fixture | Must-have/gate gap is named or accepted | | |
| Employer/role naming | `guardrails.rs` | Unit test | Letter names the employer and role | | |
| Resume-bullet grounding | `guardrails.rs`; resume path | Grounding fixture | Bullet traces to a confirmed claim | | |
| Advisory scope review | `scope_review.rs`; `guardrails.rs` | Advisory test/fixture | Non-blocking; must cite a real claim and quote the draft | | |
| Commit gate | `orchestrator.rs::commit_application` | Commit tests | Re-reads rendered documents; blocks unless override | | |
| Override | `orchestrator.rs`; UI | Override test | Written reason, timestamp and findings recorded | | |
| Disclosure gate | `orchestrator.rs`; UI | Disclosure tests | Commit refused without a per-application acknowledgement | | |
| Untrusted input | JD handling; `guardrails.rs` | Injection fixture | Posting delimited as data; warning recorded; gate holds | | |
| Claim provenance | `claims.rs`; `db.rs` | Provenance tests | Type, reference, fingerprint, verifier, confirmation time | | |
| Audit record | `compliance.rs`; `orchestrator.rs` | Audit tests | Append-only; model digest, rule hash, template and document hashes | | |

**Trace notes**

```
______________________________________________________________________________
______________________________________________________________________________
```

## 4. Code review checklist

- [ ] There is no automatic commit or submission path.
- [ ] `commit_application` re-reads the rendered documents and re-runs the gate.
- [ ] Advisory findings are excluded from the blocking decision.
- [ ] An override requires a non-empty written reason and records the finding summary.
- [ ] The per-application disclosure acknowledgement is checked against the latest generation.
- [ ] Audit events are append-only; the commit report does not overwrite the apply report.
- [ ] Model name and digest are captured at generation and commit.
- [ ] Template version and rule-set hash are recorded.
- [ ] Claims used in the audit are only those cited by the documents.
- [ ] Instruction-like posting text is delimited and cannot alter system rules.
- [ ] No hosted inference path sends document text to a third party.
- [ ] No guardrail has been weakened to make a test pass.

## 5. Release and change-control check

| Release / change | Commit | Tests run | Model digest | Rule hash | Template | Impact review | Change note | Result | Finding |
|---|---|---|---|---|---|---|---|---|---|
| | | | | | | yes / no / N/A | yes / no | | |
| | | | | | | yes / no / N/A | yes / no | | |
| | | | | | | yes / no / N/A | yes / no | | |

- [ ] Every release in the period has a test record.
- [ ] Every material change has an impact-assessment review.
- [ ] The regression run covers the guardrail fixtures.
- [ ] The release record identifies the model digest and template version.
- [ ] A non-material classification has a recorded rationale.

## 6. Evaluation fixture evidence

| Fixture / test group | Expected finding code | Expected clean result | Actual | Pass? | Note |
|---|---|---|---|---|---|
| Fabricated metric | | block | | yes / no | |
| Dropped qualifier | | block | | yes / no | |
| Scope inflation | | block | | yes / no | |
| Never-claim | | block | | yes / no | |
| Omitted gap | | block | | yes / no | |
| Ungrounded bullet | | block | | yes / no | |
| Employer mismatch | | block | | yes / no | |
| Prompt injection | | block / warning as specified | | yes / no | |
| Clean fixture | none | no findings | | yes / no | |
| Advisory citation check | `scope_review_advisory` | non-blocking | | yes / no | |

Record the total test count, fixture count and any test that is not run. A clean fixture that produces a
finding is a failure; a finding fixture that produces no finding is a failure.

## 7. Result summary and sign-off

| Result | Count | Finding IDs |
|---|---|---|
| Conforming | | |
| Observation | | |
| Minor nonconformity | | |
| Major nonconformity | | |
| Not tested / limitation | | |

**Code/test conclusion:**

```
______________________________________________________________________________
______________________________________________________________________________
```

| Role | Name | Signature | Date |
|---|---|---|---|
| Preparer | | | |
| Reviewer | | | |
