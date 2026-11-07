# Management review procedure

**Artifact ID:** C-06
**Version:** 1.0
**Effective date:** 2026-11-07, on approval
**Owner:** Accountable Owner
**Approval:** ______________________________  Date: ______________
**Status:** draft procedure; **no management review has been held**. The first review is to be
scheduled by the Accountable Owner.

## 1. Purpose

This procedure defines how top management reviews the CareerOS AI management system, its performance
and its improvement needs. It supports ISO/IEC 42001 clause 9.3 and closes the loop from monitoring,
internal audit and incidents to decisions and resources.

A management review is a decision meeting, not a status presentation. It must produce recorded
decisions, owners and dates.

## 2. Participants and quorum

| Role | Participation |
|---|---|
| Accountable Owner | Chair; accountable for decisions and resources |
| AI System Owner | Presents performance, incidents and change proposals |
| Maintainer / Operator | Presents metrics, test evidence and technical constraints |
| Independent Reviewer | Presents audit findings, if an audit was performed |
| User representative | Optional; presents feedback and concerns |

Quorum is the Accountable Owner and the AI System Owner. If one person holds both roles, the review is
still held and the combined role is recorded; an independent reviewer should attend where available.

## 3. Frequency

- At least **annually**.
- After a critical incident, a major nonconformity, a material model/rule/template change, a
  significant objective miss, or a change in scope or distribution.
- Before a decision to distribute the system beyond the operator or to add submission capability.

## 4. Inputs

1. Status of actions from the previous review.
2. Changes in internal and external issues relevant to the AI management system.
3. Changes in the needs and expectations of interested parties, including job seekers and employers.
4. AI system performance, including guardrail pass rates and false-block data.
5. Objective performance against [HONESTY-OBJECTIVES.md](HONESTY-OBJECTIVES.md), with denominators.
6. Incident and concerns trends, including appeals and non-retaliation.
7. Internal audit results and corrective-action status.
8. Results of the impact assessment and any new affected groups or risks.
9. Supplier and model changes, including model digest and known limitations.
10. Resource adequacy, competence and training.
11. User feedback and disclosure comprehension.
12. Legal, regulatory and contractual changes.
13. Opportunities for improvement.

## 5. Outputs

The review records decisions on:

- whether the AI management system remains suitable, adequate and effective;
- improvement opportunities and changes to the system, rules, templates or documentation;
- resource needs, including independent review and audit;
- changes to policy, objectives, impact assessment, risk treatment or the Statement of Applicability;
- corrective actions for nonconformities and incidents;
- whether to continue, restrict, pause or expand use of the system;
- owners and due dates for every action.

A decision to accept residual risk is recorded with rationale, the accepting role and a review date.

## 6. Agenda

1. Welcome, quorum and conflict-of-interest declarations.
2. Review of previous actions.
3. Context, scope and interested-party changes.
4. Performance and objective results.
5. Incidents, concerns and appeals.
6. Audit findings and corrective actions.
7. Impact assessment and risk treatment.
8. Supplier and model changes.
9. Resources and competence.
10. Decisions and action plan.
11. Close and next review date.

## 7. Minutes template

```
Management review
Review ID:
Date and location:
Chair:
Attendees / roles:
Quorum met?              yes / no
Conflicts declared:
Previous actions status:
Context and scope changes:
Performance and objective results (with denominators):
Incidents, concerns and appeals:
Audit findings and CAPA status:
Impact assessment and risk treatment:
Supplier / model changes:
Resources and competence:
Decisions:
  1.
  2.
Actions:
  | Action | Owner | Due | Success measure |
Next review date:
Chair signature:
```

Minutes are retained by the operating organization. They are an input to the next internal audit.

## 8. First review plan

| Item | Value |
|---|---|
| Planned date | ____________________ (to be set by the Accountable Owner) |
| Chair | Accountable Owner |
| Required inputs | Objectives baseline, incident summary, audit report (if available), impact assessment, SoA, change log |
| Expected output | First action plan and decision on whether the system remains suitable for its intended use |

**Not yet held.** Until minutes exist, this repository contains a procedure only. A management review
cannot be inferred from the 90-day honesty review; the two are separate.

## 9. Review of this procedure

Review annually with the management review cycle, or earlier if the organization's structure, scope or
independence arrangements change.
