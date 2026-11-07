# WP-02 — Document and control review

**Audit ID:** IA-________-01
**Prepared by:** ______________________________  **Date:** ______________
**Reviewed by:** ______________________________  **Date:** ______________
**Status:** Not started

[Back to workpaper index](README.md)

## 1. Purpose

Test whether the governance artifacts exist, are approved for the audit period, and support the
control-to-evidence map. This workpaper tests documents and applicability; it does not substitute for
the operating tests in WP-03 or the code tests in WP-04.

## 2. Document register

For each artifact, record the version reviewed, whether the approval block is signed, the approval
date, and whether the content matches the current system.

| Artifact | Version | Approval signed? | Approval date | Content current? | Finding ref | Notes |
|---|---|---|---|---|---|---|
| [AI-POLICY.md](../AI-POLICY.md) | | yes / no | | yes / no | | |
| [AI-RACI.md](../AI-RACI.md) | | yes / no | | yes / no | | |
| [CONCERNS-CHANNEL.md](../CONCERNS-CHANNEL.md) | | yes / no | | yes / no | | |
| [AI-IMPACT-ASSESSMENT.md](../AI-IMPACT-ASSESSMENT.md) | | yes / no | | yes / no | | |
| [HONESTY-OBJECTIVES.md](../HONESTY-OBJECTIVES.md) | | yes / no | | yes / no | | |
| [INTERNAL-AUDIT.md](../INTERNAL-AUDIT.md) | | yes / no | | yes / no | | |
| [MANAGEMENT-REVIEW.md](../MANAGEMENT-REVIEW.md) | | yes / no | | yes / no | | |
| [STATEMENT-OF-APPLICABILITY.md](../STATEMENT-OF-APPLICABILITY.md) | | yes / no | | yes / no | | |
| [OPERATOR-ADOPTION-CHECKLIST.md](../OPERATOR-ADOPTION-CHECKLIST.md) | | yes / no | | yes / no | | |
| [AI-SYSTEM-CARD.md](../AI-SYSTEM-CARD.md) | | N/A | | yes / no | | |
| [GUARDRAIL-METHOD.md](../GUARDRAIL-METHOD.md) | | N/A | | yes / no | | |
| [COMPLIANCE-EVIDENCE.md](../COMPLIANCE-EVIDENCE.md) | | N/A | | yes / no | | |

**Document review results**

- [ ] Every required artifact was provided.
- [ ] Every required approval block is signed by the named role.
- [ ] The approval date is on or before the start of the audit period, or the gap is recorded.
- [ ] No artifact claims certification or compliance.
- [ ] Version numbers and dates are consistent across documents.
- [ ] The adoption record names the AO, SO, MO and IR.
- [ ] The concerns contact and independent escalation route are filled in.
- [ ] The three excluded controls have a reason and a re-evaluation trigger.

## 3. Selected control matrix

Test all 35 selected controls for an existing evidence path. Mark `Y` only when the path exists and
supports the control; mark `N` with a finding reference. Operating effectiveness is tested in WP-03
and WP-04; this matrix is the design/document check.

| Control | Evidence path | Exists? | Operating test | Status agrees? | Note / finding |
|---|---|---|---|---|---|
| A.2.2 | AI-POLICY | | WP-02 §2 | | |
| A.2.3 | AI-POLICY §3(13), §7; host policies | | WP-05 interview | | |
| A.2.4 | AI-POLICY §9; MANAGEMENT-REVIEW | | WP-05 | | |
| A.3.2 | AI-RACI; adoption checklist | | WP-05 | | |
| A.3.3 | CONCERNS-CHANNEL; incident log | | WP-03 §5 | | |
| A.4.2 | AI-SYSTEM-CARD; AI-RACI | | WP-02 §2 | | |
| A.4.3 | System card data section; claims ledger | | WP-03 §3 | | |
| A.4.4 | AI-SYSTEM-CARD | | WP-02 §2 | | |
| A.4.5 | AI-SYSTEM-CARD | | WP-02 §2 | | |
| A.4.6 | AI-RACI; AI-POLICY §8 | | WP-05 | | |
| A.5.2 | AI-IMPACT-ASSESSMENT §2–§6 | | WP-02 §5 | | |
| A.5.3 | AI-IMPACT-ASSESSMENT; sign-off | | WP-02 §2 | | |
| A.5.4 | AI-IMPACT-ASSESSMENT §3–§4 | | WP-02 §5 | | |
| A.5.5 | AI-IMPACT-ASSESSMENT §3–§4 | | WP-02 §5 | | |
| A.6.1.2 | HONESTY-OBJECTIVES | | WP-03 §6 | | |
| A.6.1.3 | GUARDRAIL-METHOD; evaluation tests | | WP-04 §3 | | |
| A.6.2.2 | AI-SYSTEM-CARD; AI-POLICY; tests | | WP-04 §3 | | |
| A.6.2.3 | AI-SYSTEM-CARD; GUARDRAIL-METHOD | | WP-02 §2 | | |
| A.6.2.4 | `eval/guardrails.json`; `cargo test` | | WP-04 §2–§3 | | |
| A.6.2.5 | Local build / release record | | WP-04 §5 | | |
| A.6.2.6 | `/honesty-stats`; reviews | | WP-03 §6 | | |
| A.6.2.7 | AI-SYSTEM-CARD; GUARDRAIL-METHOD; SoA | | WP-02 §2 | | |
| A.6.2.8 | `applications/<id>/audit/events.jsonl` | | WP-03 §4 | | |
| A.7.3 | Setup/import; claims confirmation | | WP-03 §3 | | |
| A.7.4 | Provenance fields; weak-provenance advisory | | WP-03 §3 | | |
| A.7.5 | SHA-256 fingerprint; verifier; evidence command | | WP-03 §3 | | |
| A.7.6 | Import / JD handling | | WP-04 §3 | | |
| A.8.2 | AI-SYSTEM-CARD; disclosure | | WP-03 §4 | | |
| A.8.3 | CONCERNS-CHANNEL §3, §6 | | WP-03 §5 | | |
| A.8.4 | CONCERNS-CHANNEL §5–§7 | | WP-03 §5 | | |
| A.9.2 | AI-POLICY; AI-SYSTEM-CARD; review dialog | | WP-03 §4 | | |
| A.9.3 | HONESTY-OBJECTIVES | | WP-03 §6 | | |
| A.9.4 | AI-POLICY §4–§5; system card; disclosure | | WP-02 §2; WP-03 §4 | | |
| A.10.2 | AI-RACI | | WP-05 | | |
| A.10.3 | Model name/digest; supplier limitations | | WP-03 §8 | | |

**Summary:** controls with evidence path: ____ / 35. Controls without a path: ____. Every `N` must
have a finding ID in WP-05. The 35 selected rows above plus the 3 exclusions in §4 account for all 38
Annex A controls.

## 4. Exclusions

| Control | Stated reason | Still valid? | Re-evaluation trigger present? | Note |
|---|---|---|---|---|
| A.7.2 | No model training or enhancement | yes / no | yes / no | |
| A.8.5 | No external distribution or customer/regulator channel | yes / no | yes / no | |
| A.10.4 | No customers | yes / no | yes / no | |

- [ ] The exclusion is consistent with the actual deployment and data flows.
- [ ] If the system is distributed, hosted or trained, the exclusion is reopened.

## 5. Impact assessment and SoA review

| Check | Result | Evidence / finding |
|---|---|---|
| Job seekers are named as the affected group | yes / no | |
| Misrepresentation, automation bias, advisory over-reliance and loss of recourse are assessed | yes / no | |
| Model, rule, template, threshold, scope and incident triggers are defined | yes / no | |
| Each trigger has an action, owner and evidence | yes / no | |
| All 38 Annex A controls were considered | yes / no | |
| 35 controls are selected and 3 excluded with reasons | yes / no | |
| Evidence index matches the SoA statuses | yes / no | |
| The impact assessment has an owner sign-off | yes / no | |
| The SoA has an owner sign-off | yes / no | |

## 6. Clause-level document check

| Clause | Evidence | Documented? | Operating evidence? | Note |
|---|---|---|---|---|
| 5.2 Policy | AI-POLICY | | | |
| 5.3 Roles | AI-RACI; adoption record | | | |
| 6.1.4 / 8.4 Impact assessment | AI-IMPACT-ASSESSMENT | | | |
| 6.2 Objectives | HONESTY-OBJECTIVES | | | |
| 7.1 Resources | AI-SYSTEM-CARD; AI-RACI | | | |
| 7.2 Competence | AI-POLICY §8; AI-RACI | | | |
| 7.3 Awareness | Policy, RACI, disclosure | | | |
| 7.4 Communication | CONCERNS-CHANNEL | | | |
| 7.5 Documented information | Evidence index; records | | | |
| 8.1 Operational control | GUARDRAIL-METHOD; commit gate | | | |
| 9.1 Monitoring | Objectives; stats; reviews | | | |
| 9.2 Internal audit | INTERNAL-AUDIT; this pack | | | |
| 9.3 Management review | MANAGEMENT-REVIEW | | | |
| 10.1 Improvement | Objectives; CAPA | | | |
| 10.2 Nonconformity | Findings/CAPA | | | |

## 7. Result summary

| Result | Count | Finding IDs |
|---|---|---|
| Conforming | | |
| Observation | | |
| Minor nonconformity | | |
| Major nonconformity | | |
| Not tested / limitation | | |

**Document review conclusion:**

```
______________________________________________________________________________
______________________________________________________________________________
```

## 8. Sign-off

| Role | Name | Signature | Date |
|---|---|---|---|
| Preparer | | | |
| Reviewer | | | |
