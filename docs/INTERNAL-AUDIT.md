# Internal audit procedure

**Artifact ID:** C-05
**Version:** 1.0
**Effective date:** 2026-11-07, on approval
**Owner:** Accountable Owner
**Auditor:** Independent Reviewer (to be named)
**Approval:** ______________________________  Date: ______________
**Status:** draft procedure; **no internal audit has been performed**. This is not a certification
audit and does not establish conformity.

## 1. Purpose

This procedure defines how the operating organization audits the Veridicus AI management system and its
honesty guardrails. It supports ISO/IEC 42001 clause 9.2 and the improvement requirements in clause 10.

The audit asks a simple question: does the evidence support the claim? It is not a code review only,
and not a documentation exercise only.

## 2. Scope

The audit covers the AI management system as scoped in
[STATEMENT-OF-APPLICABILITY.md](STATEMENT-OF-APPLICABILITY.md):

- governance: policy, roles, objectives, concerns channel;
- impact assessment and risk treatment;
- data, claims and provenance;
- design, development, verification and release;
- operation, monitoring, audit records and incidents;
- supplier and model dependencies;
- user information and disclosure;
- corrective action and management review.

## 3. Independence and competence

The auditor must not have operational responsibility for the activity being audited (clause 9.2). For a
single-operator project, the Accountable Owner must appoint an external reviewer or peer. If that is not
possible, the review is labeled a **self-assessment** and cannot be presented as an independent audit.

The auditor must be able to:

- read Rust and TypeScript well enough to trace a guardrail finding to its test and audit event;
- run `cargo test` and interpret the evaluation fixtures;
- inspect the claims ledger, audit events, disclosure acknowledgements and honesty reviews;
- assess whether a documented control is actually operating.

Competence and independence are recorded before the audit starts.

## 4. Frequency and triggers

Audit at least **annually**. Add an audit when:

- the model family, digest or inference route changes materially;
- a guardrail rule, threshold or template version changes materially;
- a critical incident, confirmed systemic false claim or repeat false-block pattern occurs;
- an objective is missed for two consecutive periods;
- the system is distributed to other users or starts submitting applications;
- a legal or regulatory obligation changes.

## 5. Audit method

1. **Plan:** confirm scope, criteria, auditor independence, schedule and sampling.
2. **Document review:** policy, RACI, impact assessment, SoA, objectives, procedures, system card,
   guardrail method and evidence index.
3. **Record review:** sample audit events, disclosure acknowledgements, overrides, incidents and honesty
   reviews for the period.
4. **Code and test review:** trace each blocking rule to its implementation and test; confirm advisory
   findings cannot block; confirm the commit gate has no bypass.
5. **Reproduction:** run the full test suite and the evaluation harness; compare with the release
   record.
6. **Interviews:** walk through one approved application and one incident end to end with the operator.
7. **Report:** record conformities, observations and nonconformities with evidence.
8. **Follow-up:** verify corrective actions before closing.

### Sampling

- Audit events: all events in the period if fewer than 50; otherwise a random sample of 20 plus every
  override and every critical incident.
- Approved applications: at least 5 or 10% of the period, whichever is greater, capped at 20.
- Incidents: all critical and high reports; a sample of medium and low reports.
- Claims: at least 10 cited claims across the sampled applications.

Record the sample frame and any limitation. A sample that cannot be drawn is a finding, not a pass.

## 6. Audit checklist

| Area | What to verify | Evidence |
|---|---|---|
| Governance | Policy approved; roles named; objectives approved; concerns route reachable | AI-POLICY, AI-RACI, HONESTY-OBJECTIVES, CONCERNS-CHANNEL |
| Impact assessment | Affected groups, risks and triggers current; sign-off recorded | AI-IMPACT-ASSESSMENT |
| SoA | Every Annex A control considered; applicability justified; evidence path valid | STATEMENT-OF-APPLICABILITY |
| Claims | Confirmed claims only; provenance populated; weak provenance surfaced | claims ledger, provenance tests |
| Guardrails | Deterministic blocks enforced; advisory findings non-blocking; no commit bypass | guardrails.rs, orchestrator tests |
| Disclosure | First-run and per-application acknowledgements recorded | compliance JSONL, UI tests |
| Audit trail | Append-only events; model digest, rule hash, template and document hashes present | compliance.rs, audit rows |
| Incidents | Intake, triage, response and appeal recorded; non-retaliation stated | incident log, concerns register |
| Monitoring | Metrics computed with denominators; 90-day reviews recorded | HONESTY-OBJECTIVES, honesty reviews |
| Change control | Material changes have impact review and regression record | change log, test output |
| Supplier | Model identity/digest and limitations recorded; no unreviewed remote inference | system card, config |
| Improvement | Findings have owners, dates and effectiveness checks | CAPA register, management review |

## 7. Findings and classification

| Class | Meaning | Required response |
|---|---|---|
| **Conforming** | The control is documented and operating as described. | Record evidence. |
| **Observation** | A weakness or improvement opportunity that is not yet a nonconformity. | Owner and target date; track. |
| **Minor nonconformity** | A control is partially implemented or an isolated lapse without systemic impact. | Corrective action within 30 days; verify. |
| **Major nonconformity** | A required control is absent, ineffective or systematically failing, or evidence is unreliable. | Contain; corrective action within 10 business days; management review; verify effectiveness. |

A major nonconformity affecting truthful output or human control triggers a release stop and an impact
assessment review.

## 8. Report template

```
Internal audit report
Audit ID:
Period:
Scope and criteria:
Auditor and independence statement:
Date(s):
Sample and limitations:
Summary of evidence reviewed:
Conformities:
Observations:
Minor nonconformities:
Major nonconformities:
Conclusion (not a certification):
Corrective actions (owner, due date):
Follow-up date:
Auditor signature:
Accountable Owner acknowledgement:
```

## 9. Initial audit plan

| Item | Value |
|---|---|
| Planned date | ____________________ (to be set by the Accountable Owner) |
| Auditor | ____________________ (must be independent) |
| Period | First 12 months of operation, or since the last audit |
| Scope | Clauses 4–10 and selected Annex A controls in the SoA |
| Deliverable | Audit report, CAPA register, management-review input |
| Workpapers | [audit/README.md](audit/README.md) — planning, sampling, tests, findings and sign-off |

**Not yet performed.** The workpaper pack is blank until the audit starts; it is not evidence that an
audit occurred. Until the report exists, the repository contains a procedure and a plan, not
audit evidence. The operating organization must schedule and complete the audit before relying on
clause 9.2 in a conformity claim.

## 10. Records

Audit plans, reports, working papers, sampling records, corrective actions and follow-up evidence are
retained by the operating organization for the defined retention period. The audit report is an input
to [MANAGEMENT-REVIEW.md](MANAGEMENT-REVIEW.md).
