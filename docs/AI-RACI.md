# AI roles and responsibilities (RACI)

**Artifact ID:** C-02
**Version:** 1.0
**Effective date:** 2026-11-07, on approval
**Owner:** Accountable Owner
**Approval:** ______________________________  Date: ______________
**Status:** draft for adoption. Role holders must be named before this is used as compliance evidence.

## 1. Purpose

This document assigns responsibilities for the Veridicus AI management system and the honesty
guardrails. It supports ISO/IEC 42001 A.3.2 (AI roles and responsibilities) and A.10.2 (allocation of
responsibilities across the AI supply chain).

## 2. Roles

| Code | Role | Accountability |
|---|---|---|
| **AO** | Accountable Owner | Owns the AI management system, policy, resources and risk acceptance. Approves the impact assessment, objectives, Statement of Applicability, internal audit report and management review actions. Cannot delegate accountability for the system. |
| **SO** | AI System Owner | Owns the product and its guardrails end to end: requirements, release, monitoring, incident triage and corrective action. Reports to AO. |
| **MO** | Maintainer / Operator | Implements and tests rules, templates, prompt plumbing and guardrail code. Runs the evaluation suite, maintains the audit record and prepares review inputs. |
| **IR** | Independent Reviewer / Auditor | Conducts internal audit and challenges the system, its evidence and its risk treatment. Has no operational responsibility for the activity being audited. |
| **U** | User | Confirms claims, reviews drafts, acknowledges disclosures, approves or rejects, reports concerns and incidents. |
| **MS** | Model / Infrastructure Supplier | Supplies the local model runtime or model artifacts (for example Ollama and the selected model). Provides model identity and known limitations; does not assume the organization's accountability. |

## 3. RACI matrix

**R** = responsible for doing the work; **A** = accountable for the outcome (one per row);
**C** = consulted; **I** = informed.

| Activity | AO | SO | MO | IR | U | MS |
|---|---|---|---|---|---|---|
| Approve AI policy and review cycle | A | R | C | C | I | I |
| Maintain policy alignment with privacy, security and host policies | A | R | C | C | I | — |
| Name role holders and record appointments | A/R | C | I | I | I | — |
| Maintain the AI system inventory and system card | I | A | R | C | I | C |
| Maintain resource, tooling, compute and competence records | I | A | R | C | I | C |
| Run and document the AI system impact assessment | A | R | C | C | I | I |
| Select controls and maintain the Statement of Applicability | A | R | C | C | I | I |
| Define measurable honesty objectives | A | R | C | C | I | I |
| Elicit and specify functional and responsible-AI requirements | I | A | R | C | C | I |
| Design and implement guardrails and prompt plumbing | I | A | R | C | I | I |
| Approve a rule, threshold or template change | I | A | R | C | I | — |
| Select or update a model | A | R | C | C | I | C |
| Run verification, validation and regression evaluation | I | A | R | C | I | C |
| Approve a release for use | A | R | C | C | I | I |
| Monitor honesty metrics and the audit record | I | A | R | C | I | — |
| Triage output incidents and false-block reports | I | A | R | C | R | I |
| Operate the concerns channel and protect reporters | A | R | C | I | R | — |
| Conduct internal audit | A | C | C | R | I | — |
| Hold management review and record decisions | A/R | R | C | C | I | I |
| Track corrective and preventive actions | A | R | R | C | I | I |
| Maintain audit, incident and review records | A | R | R | C | I | — |
| Manage the model and tooling supplier relationship | A | R | C | C | I | R |
| Provide user information and disclosure | I | A | R | C | I | — |
| Stop or restrict use after a serious concern | A/R | R | C | C | I | I |

## 4. Decision rights and escalation

1. **SO** may stop a release or pause generation when a guardrail defect, incident or supplier change
   creates unacceptable risk. The stop is recorded and reported to AO.
2. **AO** accepts residual risk, approves overrides of policy, and authorizes resumption after a
   serious incident. Risk acceptance is recorded with rationale and review date.
3. **IR** may raise a nonconformity and require corrective action. IR does not approve releases.
4. **U** may decline to approve any draft and may report a concern. A user is never required to accept a
   generated statement.
5. A concern about the AO or SO is routed to IR or an external reviewer named in the adoption record.

## 5. Independence and single-operator limitation

Veridicus is currently a single-user demonstrator. The same person may hold AO, SO and MO. That is
acceptable for operation but **not** for independent audit. Before claiming an independent internal
audit, the operating organization must appoint an IR who did not perform the audited activity (an
external reviewer, peer or separate function) and record the appointment.

If no independent reviewer is available, the review is a self-assessment. It must be labeled as such
and cannot be presented as satisfying ISO/IEC 42001 clause 9.2 independence.

## 6. Appointment record

| Role | Named holder | Appointment date | Independence note |
|---|---|---|---|
| Accountable Owner | | | |
| AI System Owner | | | |
| Maintainer / Operator | | | |
| Independent Reviewer / Auditor | | | |
| Model / Infrastructure Supplier | Ollama / selected model supplier | | Third-party; see system card |

Record the appointments and independence declaration in
[OPERATOR-ADOPTION-CHECKLIST.md](OPERATOR-ADOPTION-CHECKLIST.md) §4. A role is not filled until the
named holder accepts it and the record is signed.

## 7. Review

Review this RACI annually, after a serious incident, after a material model/rule/template change, or
when the system is distributed to users other than the operator. Update the appointment record at the
same time.
