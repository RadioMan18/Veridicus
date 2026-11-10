# WP-01 — Planning, independence and sampling

**Audit ID:** IA-________-01
**Period:** ____________________ to ____________________
**Prepared by:** ______________________________  **Date:** ______________
**Reviewed by:** ______________________________  **Date:** ______________
**Status:** Not started

[Back to workpaper index](README.md)

## 1. Audit identity

| Field | Entry |
|---|---|
| Audit type | First internal audit of the Veridicus AI management system |
| Audit ID | IA-________-01 |
| Audit period | |
| Planned fieldwork start | |
| Planned fieldwork end | |
| Planned report date | |
| Auditor | |
| Auditor independence confirmed | yes / no / self-assessment |
| Accountable Owner | |
| AI System Owner | |
| Evidence location | |
| Report distribution | |

## 2. Scope and criteria

| Item | Entry |
|---|---|
| Scope | Clauses 4–10 and the 35 selected Annex A controls for the local, single-user Veridicus Desktop |
| Exclusions | Model training/fine-tuning; hosted inference; autonomous submission; screening; A.7.2, A.8.5, A.10.4 |
| Criteria | ISO/IEC 42001:2023 clauses and selected Annex A controls; adopted policy, RACI, objectives, impact assessment and SoA; system card; guardrail method; evidence index |
| Locations reviewed | `docs/`; application workspace; `compliance/`; code and tests; release record |
| Sampling plan | `README.md` §5 and this workpaper §5 |
| Prior audit | None |
| Prior findings | None |

## 3. Independence and competence

### 3.1 Independence declaration

- [ ] I did not design, implement, operate or approve the activity I am auditing.
- [ ] I have no incentive to reach a particular conclusion.
- [ ] I can access the evidence without the operator editing it first.
- [ ] I can report a major nonconformity without retaliation.
- [ ] A concern about the Accountable Owner or AI System Owner can reach me directly.

If any box is unchecked, the review is a **self-assessment**. Record the limitation here and in the
report conclusion; it cannot be presented as an independent audit under clause 9.2.

**Independence note:**

```
______________________________________________________________________________
______________________________________________________________________________
```

### 3.2 Competence record

| Skill | Evidence of competence | Date |
|---|---|---|
| Read Rust/TypeScript and trace a finding to code | | |
| Run `cargo test` and interpret fixtures | | |
| Inspect claims, audit events, disclosures and reviews | | |
| Assess whether a control is operating, not merely documented | | |
| Write findings and corrective actions | | |

## 4. Schedule and logistics

| Activity | Planned date | Actual date | Notes |
|---|---|---|---|
| Planning and evidence request | | | |
| Document and control review | | | |
| Population export and sampling | | | |
| Operating tests | | | |
| Code/test reproduction | | | |
| Interviews | | | |
| Findings review with AO/SO | | | |
| Draft report | | | |
| Sign-off | | | |
| Follow-up | | | |

## 5. Sampling worksheet

Export each population before selecting. Record the export hash and selection seed. The rules are in
`README.md` §5; this table records what was actually done.

| Population | Source / query | Population count | Required sample | Selected count | Export hash | Seed / method | Exception |
|---|---|---|---|---|---|---|---|
| Approved applications | Application records, period filter | | max(5, 10%) cap 20 | | | | |
| Audit events | `audit/events.jsonl` | | all if <50; else 20 + mandatory | | | | |
| Overrides | Audit events / `guardrail-override.md` | | all | | | | |
| Critical incidents | Incident log | | all | | | | |
| High incidents | Incident log | | all | | | | |
| Medium/low incidents | Incident log | | 20% or 5, cap 10 | | | | |
| Cited claims | Claims ledger / sampled applications | | 10 | | | | |
| Governance documents | `docs/` | | all | | | | |
| Selected controls | SoA | 35 | all mapped | | | | |
| Material changes | Change log | | all | | | | |
| Releases | Release record | | all | | | | |
| Honesty reviews | `compliance/reviews.jsonl` | | all in period | | | | |

**Mandatory inclusions**

- [ ] At least one application with an override, if any exists.
- [ ] At least one application with a false-block incident, if any exists.
- [ ] Every critical incident.
- [ ] Every override audit event.
- [ ] At least one weak-provenance claim, if any exists.

**If a mandatory item exceeds the sample cap:** record the expanded sample and the reason. Do not
drop it silently.

**Population limitations**

```
______________________________________________________________________________
______________________________________________________________________________
```

## 6. Evidence request list

Use [EVIDENCE-REQUEST.md](EVIDENCE-REQUEST.md) as the cover memo and
[EVIDENCE-MANIFEST.csv](EVIDENCE-MANIFEST.csv) for hashes and receipt. The provider supplies the full
population; the auditor selects the sample.

| # | Request | Source | Received | Provided by | Notes |
|---|---|---|---|---|---|
| 1 | Signed policy, RACI, objectives and SoA | `docs/` and adoption record | | | |
| 2 | Signed impact assessment | `docs/AI-IMPACT-ASSESSMENT.md` | | | |
| 3 | Completed adoption checklist | `docs/OPERATOR-ADOPTION-CHECKLIST.md` | | | |
| 4 | Concerns contact and test record | Adoption checklist §5 | | | |
| 5 | Application and audit-event export for the period | Workspace | | | |
| 6 | Claims ledger export and `verify_claim_evidence` output | Application DB | | | |
| 7 | Incident log / CSV export | Workspace | | | |
| 8 | Honesty reviews and metrics summaries | `compliance/` | | | |
| 9 | Release/test records and change log | Release record | | | |
| 10 | Model identity and supplier limitations | System card / audit | | | |
| 11 | Current code snapshot and `git status` | Repository | | | |
| 12 | CAPA register and prior management decisions | Organization | | | |

## 7. Risk focus for this audit

| Risk | Why it matters | Where tested |
|---|---|---|
| False or inflated claim reaches an approved document | Core harm to job seekers and employers | WP-03 §2–§4; WP-04 §3 |
| Advisory finding treated as a block, or a block bypassed | Human control and truthful output | WP-04 §3 |
| Disclosure or override not recorded | Consent and recourse | WP-03 §4 |
| Provenance is missing or unverifiable | Claims cannot be traced to evidence | WP-03 §3 |
| Incident or concern is not acknowledged | Loss of recourse | WP-03 §5; WP-05 |
| Material model/rule/template change ships without review | Behavior changes without evidence | WP-03 §7; WP-04 §5 |
| Metrics hide a miss through missing denominators | Misleading performance claims | WP-03 §6 |
| Evidence record is altered or incomplete | Audit conclusion unreliable | WP-03 §4; WP-04 §2 |

## 8. Planning approval

| Role | Name | Signature | Date |
|---|---|---|---|
| Auditor | | | |
| Accountable Owner (scope and access) | | | |

Planning is not complete until the independence declaration, competence record, schedule, sampling
worksheet and evidence requests are filled in.
