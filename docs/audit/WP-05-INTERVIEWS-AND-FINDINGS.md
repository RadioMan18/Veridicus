# WP-05 — Interviews, findings and report

**Audit ID:** IA-________-01
**Prepared by:** ______________________________  **Date:** ______________
**Reviewed by:** ______________________________  **Date:** ______________
**Status:** Not started

[Back to workpaper index](README.md)

## 1. Interviews

Interview the role holders named in the adoption record. If a role is vacant, record the interim
arrangement and treat the gap under the RACI/competence criteria.

| Role | Name | Date | Duration | Notes / evidence |
|---|---|---|---|---|
| Accountable Owner | | | | |
| AI System Owner | | | | |
| Maintainer / Operator | | | | |
| Independent Reviewer | | | | |
| User representative (if available) | | | | |

### Accountable Owner

1. What are you accountable for in the AI management system, and where is that recorded?
2. How do you approve the policy, objectives and SoA, and what evidence would show it?
3. How do you accept residual risk, and when did you last do so?
4. What resources have you provided for independent review and audit?
5. How will the first management review be scheduled, and what decisions must it make?
6. What would make you stop or restrict use of the system?

### AI System Owner

1. Walk through one approved application from generation to approval. Where are the disclosure,
   gate verdict and override recorded?
2. How do you decide whether a model, rule or template change is material?
3. How are incidents triaged, and who can see them?
4. Which objectives are measured, with what denominators, and where are the results?
5. How are false blocks distinguished from valid blocks?
6. What supplier limitations are known, and how are they recorded?
7. How would you detect a missed false claim after approval?

### Maintainer / Operator

1. How does a rule change get from edit to release, and what tests run?
2. Where is the rule-set hash and template version recorded?
3. How is claim provenance captured and verified?
4. Show the audit event for a generation and a commit. What fields prove what happened?
5. How is the working tree snapshotted for an audit?
6. What happens when a test fails or a guardrail is weakened?

### Independent Reviewer

1. How is your independence from operations established and recorded?
2. What access did you have to code, records and people?
3. What would you classify as a major nonconformity?
4. How will findings be tracked and verified?
5. If you also hold an operational role, how is the self-assessment limitation disclosed?

### User representative

1. What did you understand the system would and would not do?
2. Did you read the findings before approving? What did you do with a blocking finding?
3. Could you find the incident-reporting action and the disclosure?
4. Did you feel able to decline a draft or report a concern without pressure?
5. What would have helped you judge whether a claim was true?

## 2. End-to-end walkthrough

Perform one approved application and one incident walkthrough with the operator.

| Walkthrough | Item ID | Steps observed | Evidence seen | Gap / finding |
|---|---|---|---|---|
| Approved application | | | | |
| Incident / concern | | | | |

- [ ] The application walkthrough reaches the audit event and the approved documents.
- [ ] The incident walkthrough reaches intake, acknowledgement, investigation, outcome and appeal.
- [ ] The operator can explain what the gate checked and what it did not check.
- [ ] The operator can explain the difference between a block, an advisory and an override.

## 3. Findings log

Classify each finding under INTERNAL-AUDIT §7. A finding needs a criterion, condition, evidence, cause
and effect. “Documentation could be better” is an observation, not a nonconformity.

| ID | Class | Criterion | Condition | Evidence | Cause | Effect | Owner | Due | Status | Verification |
|---|---|---|---|---|---|---|---|---|---|---|
| F-01 | conforming / observation / minor NC / major NC | | | | | | | | open / closed | |
| F-02 | | | | | | | | | | |
| F-03 | | | | | | | | | | |
| F-04 | | | | | | | | | | |
| F-05 | | | | | | | | | | |

**Classification reminders**

| Class | Required response |
|---|---|
| Conforming | Record evidence; no action required. |
| Observation | Owner and target date; track to closure. |
| Minor nonconformity | Corrective action within 30 days; verify. |
| Major nonconformity | Contain; corrective action within 10 business days; management review; verify effectiveness. |

A major nonconformity affecting truthful output or human control triggers a release stop and an
impact-assessment review.

## 4. Corrective and preventive action log

| CAPA ID | Finding | Action | Root cause addressed? | Owner | Due | Evidence of completion | Effectiveness check | Closed |
|---|---|---|---|---|---|---|---|---|
| CAPA-01 | | | yes / no | | | | | yes / no |
| CAPA-02 | | | yes / no | | | | | yes / no |
| CAPA-03 | | | yes / no | | | | | yes / no |

Do not close a CAPA because a document was edited. Close it when the corrected condition is verified.

## 5. Audit report skeleton

```
Internal audit report
Audit ID:
Period:
Scope and criteria:
Auditor and independence statement:
Date(s):
Executive summary:
Sample and limitations:
Evidence reviewed:
Conformities:
Observations:
Minor nonconformities:
Major nonconformities:
Overall conclusion (not a certification):
Corrective actions (owner, due date):
Follow-up date:
Auditor signature:
Accountable Owner acknowledgement:
```

### Overall conclusion options

- [ ] Conforming — no nonconformities found; observations listed.
- [ ] Conforming with observations — no nonconformities, but improvement actions recorded.
- [ ] Minor nonconformity — one or more minor NCs; corrective action required.
- [ ] Major nonconformity — one or more major NCs; containment and management review required.
- [ ] Unable to conclude — evidence is insufficient; state exactly what is missing.

The conclusion must state that it is an internal audit conclusion and **not** a certification or a
declaration of ISO/IEC 42001 conformity.

## 6. Management review input

| Input | Summary | Decision / action needed |
|---|---|---|
| Major/minor nonconformities | | |
| Observations and trends | | |
| Objective misses | | |
| Incident/concern trends | | |
| Resource or independence gaps | | |
| Impact assessment / SoA changes | | |
| Supplier/model changes | | |

## 7. Sign-off

| Role | Name | Signature | Date |
|---|---|---|---|
| Preparer | | | |
| Reviewer | | | |

Final sign-off is in [WP-06](WP-06-SIGNOFF.md).
