# Operator adoption checklist

**Artifact ID:** C-09
**Version:** 1.0
**Effective date:** 2026-11-07
**Owner:** Accountable Owner
**Status:** blank operating record. It becomes evidence of adoption only when completed and signed.
Completing it does not certify CareerOS or establish ISO/IEC 42001 conformity.

## 1. Purpose

This checklist turns the draft governance artifacts into an operating record. It records:

- who holds each governance role;
- where concerns are reported and who monitors the route;
- that the policy, RACI, objectives and Statement of Applicability were reviewed and approved;
- the supporting reviews that must happen before those approvals;
- the actions required in the first 90 days.

Use it with [AI-RACI.md](AI-RACI.md), [CONCERNS-CHANNEL.md](CONCERNS-CHANNEL.md),
[HONESTY-OBJECTIVES.md](HONESTY-OBJECTIVES.md) and
[STATEMENT-OF-APPLICABILITY.md](STATEMENT-OF-APPLICABILITY.md). The source artifacts keep their own
approval blocks; this checklist is the consolidated record.

## 2. Completion rules

1. Complete the sections in order. A later signature does not cure an earlier blank that the artifact
   requires.
2. Do not invent a name, contact, date or signature. Write `N/A` only with a reason.
3. Do not backdate. If an artifact changes after approval, record the new version and re-approve it.
4. Sign the approval block in each source artifact **and** record the approval in §6. The signature is
   the approver's own; an assistant may prepare the record but may not sign for the approver.
5. Store the signed checklist and the signed artifacts in the organization's evidence location, not
   only in the Git working tree. Do not place credentials or personal data in this record.
6. Mark the adoption decision in §9. If any required approval is missing or conditional, the decision
   cannot be “approved without conditions.”
7. This record is evidence of an internal adoption decision. It is not an accreditation certificate
   and must not be described as one.

## 3. Organization and scope

| Field | Entry |
|---|---|
| Operating organization / legal entity | |
| System name and version | CareerOS Desktop, version: |
| Repository or workspace used | |
| Scope of the AI management system | Local, single-user career-document drafting and honesty guardrails |
| Explicit exclusions | No model training, hosted inference, autonomous submission, screening or customer distribution |
| Adoption record ID | |
| Adoption date | |
| Evidence location | |
| Next scheduled policy review | |
| Next scheduled management review | |

## 4. Governance roles

Record the named holder and the date they accepted the role. A role may be held by the same person,
but the independence rule for the auditor still applies.

| Role | Named holder | Appointment date | Acceptance recorded | Notes |
|---|---|---|---|---|
| Accountable Owner (AO) | | | yes / no | Owns the AIMS and signs the policy, objectives and SoA |
| AI System Owner (SO) | | | yes / no | Owns the product, guardrails, incidents and monitoring |
| Maintainer / Operator (MO) | | | yes / no | Implements rules, runs tests, maintains records |
| Independent Reviewer / Auditor (IR) | | | yes / no | Must not have operational responsibility for audited work |
| User representative (optional) | | | yes / no | Provides user feedback and concerns |

**Independence declaration**

- [ ] The IR is a different person from the AO, SO and MO for the activities being audited.
- [ ] If the IR also holds an operational role, the review will be labeled a **self-assessment**, not
      an independent audit, and an external or peer reviewer will be used for the first audit.
- [ ] A concern about the AO or SO can reach the IR without passing through that person.
- [ ] The IR's competence to review the guardrails, records and tests is recorded.

**Decision-rights confirmation**

- [ ] The AO accepts residual risk and approves resumption after a serious incident.
- [ ] The SO can stop a release or pause generation and must report the stop to the AO.
- [ ] The IR can raise a nonconformity but does not approve releases.
- [ ] The user can decline any draft and report a concern without retaliation.

## 5. Concerns channel

### 5.1 In-app output channel

The in-app **Report a problem with this output** action is the primary route for output-specific
incidents. It writes a local structured row and supports CSV export.

| Check | Entry |
|---|---|
| Test date | |
| Test application ID | |
| Category used | `false_block` / `missed_false_claim` / `scope_error` / `wrong_evidence` / `other` |
| Incident ID produced | |
| CSV export verified | yes / no |
| Tester | |
| Result and any defect | |

- [ ] The review dialog exposes the incident category, note and incident ID.
- [ ] The export opens and contains the test incident.
- [ ] The tester knows that the in-app record is local and must be sent to the operator.

### 5.2 External and governance route

| Field | Entry |
|---|---|
| Concerns contact (email, form or tracker) | |
| Who monitors it | |
| Backup monitor | |
| Availability / response expectation | |
| Independent escalation for concerns about AO/SO | |
| Accessibility or language arrangements | |
| Confidentiality and non-retaliation briefing date | |

- [ ] The contact is monitored and is not a personal social-media account.
- [ ] The contact can receive a report from a user, an employer or a named third party.
- [ ] A test message was sent and acknowledged within the severity target.
- [ ] The reporter can request confidentiality or anonymity where the route allows it.
- [ ] A conflict of interest is disclosed and the person concerned does not investigate their own report.
- [ ] The appeal route to the IR is stated in the acknowledgement.

### 5.3 Triage readiness

- [ ] The SO or delegate can assign severity using CONCERNS-CHANNEL §5.
- [ ] Critical reports have an immediate escalation path to the AO.
- [ ] Incident records capture containment, root cause, corrective action, owner and due date.
- [ ] The incident log can be exported for the 90-day review and internal audit.

## 6. Required approvals

The four required approvals are the policy, RACI, objectives and Statement of Applicability. Sign each
source artifact's approval block as well as this table.

| Artifact | Version approved | Approver role | Name | Signature | Date | Decision |
|---|---|---|---|---|---|---|
| [AI-POLICY.md](AI-POLICY.md) | 1.0 | Accountable Owner | | | | approved / approved with conditions / not approved |
| [AI-RACI.md](AI-RACI.md) | 1.0 | Accountable Owner | | | | approved / approved with conditions / not approved |
| [HONESTY-OBJECTIVES.md](HONESTY-OBJECTIVES.md) | 1.0 | Accountable Owner | | | | approved / approved with conditions / not approved |
| [STATEMENT-OF-APPLICABILITY.md](STATEMENT-OF-APPLICABILITY.md) | 1.0 | Accountable Owner | | | | approved / approved with conditions / not approved |

**Conditions or exceptions**

| Artifact | Condition / exception | Owner | Due date | Closed |
|---|---|---|---|---|
| | | | | yes / no |
| | | | | yes / no |
| | | | | yes / no |

### 6.1 Supporting reviews

These are not substitutes for the four approvals. They are the reviews that make the approvals
credible.

| Artifact | Reviewer role | Name | Review date | Outcome / actions |
|---|---|---|---|---|
| [AI-IMPACT-ASSESSMENT.md](AI-IMPACT-ASSESSMENT.md) | AI System Owner / AO | | | |
| [CONCERNS-CHANNEL.md](CONCERNS-CHANNEL.md) | AI System Owner | | | |
| [INTERNAL-AUDIT.md](INTERNAL-AUDIT.md) | Accountable Owner / IR | | | |
| [MANAGEMENT-REVIEW.md](MANAGEMENT-REVIEW.md) | Accountable Owner | | | |

## 7. Pre-adoption checks

**Policy and alignment**

- [ ] The policy's intended and prohibited uses match the system card and the actual product.
- [ ] The policy is aligned with the host organization's privacy, security, retention and
      acceptable-use policies; conflicts and resolutions are recorded.
- [ ] The policy review date and triggers are set.
- [ ] The AI-assisted disclosure and human-approval rule match the implemented UI.

**RACI and independence**

- [ ] Every role in AI-RACI §2 has a named holder or a recorded vacancy with an interim arrangement.
- [ ] The appointment record in AI-RACI §6 is updated.
- [ ] The single-operator limitation is disclosed, and an independent reviewer is named for audit.

**Objectives**

- [ ] The AO accepts the targets, denominators and sample rules.
- [ ] The data sources for H2, H3, H5 and H8 exist and are readable.
- [ ] A baseline measurement owner and date are set.
- [ ] The CAPA path for a missed target is understood.

**Impact assessment and SoA**

- [ ] Job seekers are recorded as the affected group, with the four primary risks and treatments.
- [ ] The material-change triggers for model, rule, template and threshold changes are accepted.
- [ ] All 38 Annex A controls were reviewed; the three exclusions and their triggers are accepted.
- [ ] Every selected control has an evidence path that exists.
- [ ] Any status changed from the draft is recorded with a reason.

**Records**

- [ ] The evidence location is access-controlled and backed up.
- [ ] The retention period is set.
- [ ] The signed artifacts and this checklist are stored together.
- [ ] The evidence index is updated from “approval pending” to “adopted” only after signature.

## 8. First 90 days

| Action | Owner | Due | Evidence |
|---|---|---|---|
| Communicate the policy, RACI and concerns route to maintainers and users | AO/SO | | Acknowledgement or briefing record |
| Measure the first objective baseline and record denominators | SO/MO | | Metrics summary |
| Test the external concerns route and record acknowledgement time | SO | | Test report |
| Schedule the first internal audit with an independent reviewer | AO | | Audit plan; [audit/README.md](audit/README.md) |
| Schedule the first management review | AO | | Calendar invitation / agenda |
| Review weak-provenance claims and set a re-confirmation plan | MO | | Claims report |
| Check the model digest and record the current model identity | MO | | System-card / audit note |
| Update the evidence index and SoA statuses after adoption | SO | | Revised index |

## 9. Adoption decision

| Field | Entry |
|---|---|
| Decision | approved / approved with conditions / not approved |
| Conditions (if any) | |
| AO name | |
| AO signature | |
| Date | |
| Next review date | |

Approval means the organization has adopted these artifacts for the stated scope. It does not mean
the system is certified, that an independent audit has occurred, or that every control is fully
implemented.

## 10. Change control

- A change of role holder or concerns contact requires an update to this checklist and the relevant
  appointment record; re-sign the affected section.
- A material change to the policy, RACI, objectives or SoA requires a new version, a new approval and
  an update to the evidence index.
- An incident, audit finding or missed objective may trigger a review under the relevant artifact;
  record the trigger and outcome here.
- Retain superseded versions with their dates. Do not overwrite a signed record.
