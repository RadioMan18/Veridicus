# AI concerns and incident reporting channel

**Artifact ID:** C-03
**Version:** 1.0
**Effective date:** 2026-11-07, on approval
**Owner:** AI System Owner
**Independent escalation:** ______________________________
**Approval:** ______________________________  Date: ______________
**Status:** draft for adoption. The concerns contact and independent escalation route must be filled in
by the operating organization before this is treated as an operating channel.

## 1. Purpose

This procedure gives users, maintainers, affected third parties and others a way to report a concern
about Veridicus or its outputs, and defines how reports are handled. It supports ISO/IEC 42001 A.3.3
(reporting of concerns), A.8.3 (external reporting) and A.8.4 (communication of incidents).

A concern is broader than a bug. It includes a false or misleading output, a missed false claim, a
false block, a privacy or security issue, a discriminatory effect, an instruction-injection attempt,
or a concern about how the system is governed.

## 2. Who may report

Anyone may report: the user, the operator, a maintainer, an employer or recruiter who received a
document, a person named in a claim, or a member of the public. Reporters do not need to be customers
or employees.

## 3. Channels

### 3.1 Primary output-specific channel: in-app incident report

In the review dialog, select **Report a problem with this output**, choose a category, add a note and
submit. The app writes a structured row to the application's `reported-problems.jsonl` with an
incident ID, timestamp, application ID, category, status and note. The incident log can be exported as
CSV.

Categories: `false_block`, `missed_false_claim`, `scope_error`, `wrong_evidence`, `other`.

This channel is local to the user's workspace. The user must send the exported CSV, or grant access to
the workspace, for the operator to receive it.

### 3.2 Governance, retaliation and non-output concerns

Use the operating organization's designated concerns contact:

**Concerns contact:** ______________________________
**Independent escalation (if the concern involves the AI System Owner or Accountable Owner):**
______________________________

The contact may be an email address, an issue tracker or another documented route. It must be
monitored, accessible and recorded in the adoption record. Do not use a personal social-media account.

### 3.3 Direct escalation

A maintainer may raise a concern directly with the Accountable Owner. A concern about the Accountable
Owner goes to the Independent Reviewer. If no independent route exists, the operator must record that
limitation and use an external reviewer before treating the channel as independent.

## 4. What to include

- Date and system version.
- Application ID, if the concern is about an output.
- Category and a clear description of what happened.
- Why it matters: who could be harmed and how.
- Whether the output was submitted or shared.
- Any evidence (exported incident CSV, audit event, document, screenshot).
- Whether the reporter wants confidentiality or anonymity.
- A safe contact route, if follow-up is wanted.

A reporter may submit anonymously where the channel allows it. Anonymous reports are still triaged,
but the organization may be unable to ask follow-up questions.

## 5. Triage and response

| Severity | Examples | Acknowledge | Contain / resolve target |
|---|---|---|---|
| **Critical** | Imminent harm, exposed personal data, credible legal or safety issue, systemic false claim already submitted | 1 business day | Contain immediately; resolve target 7 days |
| **High** | Missed false claim, repeated false blocks, scope inflation in an approved document | 5 business days | 30 days |
| **Medium** | Isolated wrong evidence, advisory over-reliance, unclear disclosure | 10 business days | 60 days |
| **Low** | Wording, usability or documentation concern without material harm | 10 business days | 90 days |

The AI System Owner assigns an owner, records the severity and rationale, investigates, and records
the outcome. A critical concern is reported to the Accountable Owner immediately and considered for a
management-review trigger.

If the reporter disagrees with the outcome, they may ask for review by the Independent Reviewer. The
appeal and its outcome are recorded.

## 6. Confidentiality and non-retaliation

- Access to a concern is limited to the people needed to investigate it.
- The organization does not retaliate against a good-faith reporter, including a maintainer who reports
  a guardrail defect or a user who reports a false output.
- Personal data in a concern is used only for handling the concern and meeting legal obligations.
- A conflict of interest is disclosed; the person concerned does not investigate their own report.
- Statutory or regulatory notification is the Accountable Owner's decision, with legal advice where
  needed. This procedure does not create a notification obligation that does not otherwise exist.

## 7. Records and learning

The operator records, at minimum:

- incident ID and date received;
- channel and reporter type (without unnecessary personal data);
- category, severity and affected application(s);
- containment, investigation, root cause and resolution;
- corrective action, owner and due date;
- communication to the reporter and any appeal;
- whether the concern triggers an impact-assessment review, policy review or management review.

Records are retained for the period set by the operating organization and are summarized in the
honesty review and internal audit. Trends feed the measurable objectives in
[HONESTY-OBJECTIVES.md](HONESTY-OBJECTIVES.md).

## 8. Report template

```
Concern / incident report
Date:
Reporter (optional):
Contact route (optional):
Confidential?             yes / no
Anonymous?                yes / no
System version:
Application ID (if any):
Category:                 false_block / missed_false_claim / scope_error / wrong_evidence / other
Severity (reporter view): critical / high / medium / low
What happened:
Who may be affected and how:
Was the output submitted or shared?
Evidence attached:
Desired outcome:
```

## 9. Review

Review this procedure annually, after a serious incident, and whenever the channel, contact or
escalation route changes. Test the channel at least once per review cycle and record the test.
