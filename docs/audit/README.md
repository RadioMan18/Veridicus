# First internal audit workpapers

**Audit ID:** IA-________-01 (assign at planning)
**Status:** **Not started.** These are blank workpapers, not audit evidence.
**Audit period:** ____________________ to ____________________
**Auditor:** ______________________________
**Prepared by:** ______________________________
**Reviewed by:** ______________________________
**Version:** 1.0 (2026-11-07)

**Governing procedure:** [INTERNAL-AUDIT.md](../INTERNAL-AUDIT.md)
**Evidence index:** [COMPLIANCE-EVIDENCE.md](../COMPLIANCE-EVIDENCE.md)
**Adoption record:** [OPERATOR-ADOPTION-CHECKLIST.md](../OPERATOR-ADOPTION-CHECKLIST.md)
**Scope and controls:** [STATEMENT-OF-APPLICABILITY.md](../STATEMENT-OF-APPLICABILITY.md)

> This is an internal audit workpaper pack. Completing it does not certify CareerOS or establish
> ISO/IEC 42001 conformity. A conclusion is written only after the evidence is tested and the
> sign-off in WP-06 is complete.

## 1. Purpose

This pack turns the procedure in `INTERNAL-AUDIT.md` into an executable first audit. It covers:

- planning, independence, competence and sampling (WP-01);
- document and control review against the evidence index (WP-02);
- operating tests of applications, claims, disclosures, overrides, audit events, incidents,
  monitoring and change control (WP-03);
- code, guardrail and test reproduction (WP-04);
- interviews, findings, corrective actions and the report skeleton (WP-05);
- completion, conclusion and sign-off (WP-06).

## 2. Scope

**In scope:** the AI management system for the local, single-user CareerOS Desktop application,
including policy, roles, impact assessment, selected Annex A controls, claims and provenance,
guardrails, disclosure, audit records, incidents, monitoring, suppliers, change control and
improvement.

**Out of scope:** model training or fine-tuning, hosted or multi-tenant inference, autonomous
submission, employment screening, legal advice and the three excluded Annex A controls (A.7.2, A.8.5,
A.10.4).

**Criteria:**

- ISO/IEC 42001:2023 clauses 4–10 and the 35 selected Annex A controls.
- The organization's adopted policy, RACI, objectives, impact assessment and SoA.
- `AI-SYSTEM-CARD.md` and `GUARDRAIL-METHOD.md` as the system's stated behavior.
- `COMPLIANCE-EVIDENCE.md` as the control-to-evidence map.
- The organization's own legal, privacy and retention obligations, where applicable.

## 3. Evidence sources

| Source | Location | Use |
|---|---|---|
| Governance artifacts | `docs/` | Policy, roles, objectives, impact, SoA, procedures |
| Adoption record | `docs/OPERATOR-ADOPTION-CHECKLIST.md` | Signed roles, contacts, approvals |
| Claims ledger | Encrypted application database | Claim provenance and confirmation |
| Application records | `applications/<id>/` | Documents, reports, overrides, audit events |
| Audit events | `applications/<id>/audit/events.jsonl` | Generation, disclosure, approval, override |
| Compliance records | `compliance/` | `reviews.jsonl`, `disclosures.jsonl` |
| Incident records | `reported-problems.jsonl`; CSV export | Intake, category, status, resolution |
| Code and tests | `src-tauri/src/`, `src-tauri/eval/`, frontend | Control implementation and regression evidence |
| Test/build output | Captured in WP-04 | Reproduction and release evidence |
| Git history | Repository | Version snapshot; not a tamper-proof record |
| Release/change records | Organization's release record | Material changes and impact review |

The audit must record whether the working tree was clean at the snapshot. If it was not, archive or
hash the reviewed tree and audit that snapshot; do not audit a moving target.

## 4. Workpaper index

| Workpaper | Title | Prepared by | Reviewed by | Status |
|---|---|---|---|---|
| [WP-01](WP-01-PLANNING.md) | Planning, independence and sampling | | | Not started |
| [WP-02](WP-02-DOCUMENT-REVIEW.md) | Document and control review | | | Not started |
| [WP-03](WP-03-OPERATING-TESTS.md) | Operating tests and samples | | | Not started |
| [WP-04](WP-04-CODE-AND-TEST.md) | Code, guardrail and test reproduction | | | Not started |
| [WP-05](WP-05-INTERVIEWS-AND-FINDINGS.md) | Interviews, findings and report | | | Not started |
| [WP-06](WP-06-SIGNOFF.md) | Completion, conclusion and sign-off | | | Not started |

**Evidence request package:** [EVIDENCE-REQUEST.md](EVIDENCE-REQUEST.md) is the cover memo and item
specification; [EVIDENCE-MANIFEST.csv](EVIDENCE-MANIFEST.csv) tracks receipt, hashes and acceptance.
The request is a template until issued and is not itself audit evidence.

## 5. Sampling summary

The detailed worksheet is in WP-01 §5. The mandatory minimums are:

| Population | Source | Sample rule | Workpaper |
|---|---|---|---|
| Approved applications | Application records | At least 5 or 10% of the period, whichever is greater, capped at 20. Include at least one override and one false-block application if present. | WP-03 §2 |
| Audit events | `audit/events.jsonl` | All if fewer than 50; otherwise 20 random plus every override and every critical incident. | WP-03 §4 |
| Incidents | Incident log / CSV | All critical and high; 20% or 5 of medium/low, whichever is greater, capped at 10. | WP-03 §5 |
| Claims | Claims ledger / documents | At least 10 cited claims across sampled applications; include a weak-provenance claim if present. | WP-03 §3 |
| Governance documents | `docs/` | All artifacts and the adoption record. | WP-02 §2 |
| Selected controls | SoA / evidence index | All 35 mapped; operating evidence for the sampled items. | WP-02 §3; WP-03; WP-04 |
| Material changes | Change log | All material changes in the period. | WP-03 §7 |
| Honesty reviews | `compliance/reviews.jsonl` | All reviews in the period, or the most recent if none fall in the period. | WP-03 §6 |
| Releases | Release/test record | All releases in the period. | WP-04 §2 |

If a population is smaller than the minimum, audit the whole population and record the limitation. If
a population is empty, record “no population” and state the scope limitation; do not substitute a
synthetic example for operating evidence.

## 6. Selection rules

1. Export the population before selecting. Record the source, export date, count and a SHA-256 hash of
   the export.
2. Use a documented random method (for example, a script with a recorded seed). Record the seed, the
   script or tool, and the selected IDs.
3. Add mandatory-risk items (overrides, critical incidents, weak-provenance claims) before random fill.
   If mandatory items exceed the cap, audit them and record the expanded sample.
4. Do not replace a selected item because it is difficult to obtain. Record the exception and treat
   missing evidence as a limitation or finding.
5. Keep the population export and selection list with the workpapers. They are part of the audit
   evidence.

## 7. Audit sequence

1. Complete WP-01 and confirm independence.
2. Complete WP-02 document review and the control matrix.
3. Export populations and select samples; complete WP-03.
4. Snapshot the reviewed tree and complete WP-04.
5. Conduct interviews and record findings in WP-05.
6. Complete WP-06, write the conclusion and obtain sign-off.
7. Schedule follow-up for every corrective action.

A workpaper is complete only when every required field has an entry, every sample is accounted for,
and every exception has a finding or a recorded rationale.
