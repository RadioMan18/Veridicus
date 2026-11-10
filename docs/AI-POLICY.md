# AI policy — Veridicus honesty guardrails

**Policy ID:** C-01
**Version:** 1.0
**Effective date:** 2026-11-07, on approval
**Owner:** Accountable Owner, Veridicus AI management system
**Approval:** ______________________________  Date: ______________
**Review:** annually, and on any trigger in §9
**Status:** draft for adoption by the Veridicus demonstrator. This policy is not a certification and
does not by itself establish conformity with ISO/IEC 42001.

## 1. Purpose

This policy states how the operating organization develops, provides and uses Veridicus Desktop, a
local, single-user job-application drafting tool. It sets the rules that the application's honesty
guardrails implement and the behavior expected of the people who operate, maintain and use it.

The policy exists because a fluent generated document can cause real harm: a job seeker can
misrepresent their experience, an employer can receive a false statement, and a named colleague can
be mischaracterized. The policy therefore makes truthfulness, human responsibility and traceability
non-negotiable.

## 2. Scope

**In scope**

- Veridicus Desktop and its local generation, claims-ledger, guardrail, audit and disclosure features.
- The operating organization's roles, reviews, records and improvement activities for the system.
- Local Ollama models and other tools used to generate or verify documents.
- The job posting, master resume and confirmed claims processed by the application.

**Out of scope**

- Training, fine-tuning or evaluating the underlying language model. Veridicus does not train a model.
- Hosted inference, multi-tenant hosting or submission of applications on a user's behalf.
- Employment screening, ranking, hiring or promotion decisions. Veridicus is not a screening tool.
- Legal advice, immigration advice or verification of a user's evidence.

## 3. Policy statements

1. **The claims ledger is the only factual authority.** Generated documents may use only claims the
   user confirmed. The base resume supplies structure and phrasing, not facts. A statement that is not
   supported by a confirmed claim must be omitted, flagged, or returned for human input.
2. **The deterministic gate decides.** Never-claim, ask-first, exact-wording, metric-qualifier, scope,
   gap-naming, employer/role and grounding checks are authoritative. Model output, including the
   advisory scope review, is a proposal and cannot override a deterministic finding.
3. **The user remains responsible.** Veridicus produces drafts. It does not submit applications. The user
   must review the documents, acknowledge the AI-assisted disclosure, and decide whether to approve.
4. **No autonomous commitment.** The generation pipeline does not commit or submit. A blocking finding
   stops approval unless the user records a written override, which is evidence of a human decision,
   not a finding that the document is true.
5. **Provenance is recorded.** A confirmed claim carries an evidence type, reference, fingerprint,
   verifier and confirmation time. Weak provenance is surfaced to the user and must not be described as
   verified evidence.
6. **The record is append-only.** Generation, disclosure acknowledgement, approval and override events
   are written to the application audit trail. Model name and digest, template version, rule hash and
   document hashes are retained so a decision can be reconstructed.
7. **Data stays local by default.** Claims and application records are stored in an encrypted local
   database. Inference uses a local Ollama instance. Veridicus does not send document text to a hosted
   model. Any future remote processing requires a new impact assessment and user disclosure.
8. **Untrusted input stays untrusted.** A job posting is data, never instructions. Instruction-like
   text is delimited, detected where possible, and recorded as a warning; it must not change the
   system rules or authorize a claim.
9. **Concerns are protected.** Anyone may report a concern or a bad output without retaliation. Reports
   are triaged under the concerns procedure; the in-app incident record is the primary output-specific
   channel.
10. **Change is controlled.** A material model, rule, template, threshold or scope change requires a
    regression run, an impact-assessment review and an audit-record note before release.
11. **Third-party components are accountable.** The operating organization remains accountable for
    model behavior even when the model is supplied by Ollama or another provider. Supplier identity,
    model digest and known limitations are recorded.
12. **The system is improved, not defended.** Incidents, false blocks, overrides, audit findings and
    objective misses feed corrective action and management review.
13. **Alignment with other policies.** This policy is read with the operating organization's privacy,
    security, retention and acceptable-use policies. If they conflict, the more protective requirement
    applies; the Accountable Owner records the resolution.

## 4. Intended use

- One user, on their own computer, preparing applications for their own career.
- Drafting a resume and cover letter from facts the user has confirmed.
- Reviewing a generated draft against its claims, gaps and guardrail findings.
- Recording what was generated and why it was approved.

## 5. Prohibited and out-of-scope uses

- Fabricating or exaggerating experience, metrics, credentials, scope or eligibility.
- Submitting an application without human review.
- Using another person's resume or data without their knowledge and authority.
- Screening, ranking, rejecting or making employment decisions about other people.
- Inferring protected characteristics or using the system to discriminate.
- Describing Veridicus as certified, compliant or a substitute for legal or professional advice.
- Removing or weakening a guardrail to make a draft pass without recording the change and its review.
- Sending claims or documents to a hosted model or third party without a new impact assessment and
  disclosure.

## 6. Human oversight

- First launch requires acknowledgement of the AI-assisted disclosure and intended use.
- Every application requires a per-application acknowledgement before approval.
- The review screen shows findings beside the documents and distinguishes blocking from advisory.
- A blocking finding requires a written override reason; the override is recorded with its findings.
- The user can report a structured incident and export the incident log.
- The honesty record is reviewed on a 90-day cadence, with the reviewer and observed counts recorded.

## 7. Data and records

- Claims, rules, gaps and application records: encrypted local SQLCipher database.
- Generated documents, audit events and incident rows: the local Veridicus workspace.
- Disclosure acknowledgements and honesty reviews: `compliance/` in the workspace.
- Git history: a versioned snapshot of approved materials, not a tamper-proof record.
- Retention: controlled by the operating organization and the user; no automatic expiry. The
  organization must define a retention period before representing the record as audit evidence.

## 8. Roles and competence

Roles and responsibilities are defined in [AI-RACI.md](AI-RACI.md). The operating organization must
name the Accountable Owner, AI System Owner, Maintainer/Operator and Independent Reviewer before this
policy is treated as approved. Where one person holds several roles, the independence limitation is
recorded and an external or peer reviewer is used for internal audit.

People who maintain rules, templates or guardrails must be able to explain the deterministic checks,
run the test suite, read an audit event and follow the incident procedure. Training and competence
records are maintained by the operating organization.

## 9. Review triggers

Review this policy at least annually and whenever:

- the model family, model digest or inference provider changes materially;
- a guardrail rule, threshold or template version changes;
- the application begins submitting applications or handling another person's data;
- a serious incident, missed false claim or pattern of false blocks is found;
- the impact assessment, Statement of Applicability, objectives or audit findings change;
- a legal, regulatory or contractual obligation changes;
- the system is distributed to users other than the operator.

## 10. Related artifacts

- [AI-RACI.md](AI-RACI.md) — roles and responsibilities.
- [CONCERNS-CHANNEL.md](CONCERNS-CHANNEL.md) — reporting and escalation.
- [AI-IMPACT-ASSESSMENT.md](AI-IMPACT-ASSESSMENT.md) — impacts, affected groups and review triggers.
- [HONESTY-OBJECTIVES.md](HONESTY-OBJECTIVES.md) — measurable objectives.
- [INTERNAL-AUDIT.md](INTERNAL-AUDIT.md) and [MANAGEMENT-REVIEW.md](MANAGEMENT-REVIEW.md) — evaluation
  and improvement.
- [STATEMENT-OF-APPLICABILITY.md](STATEMENT-OF-APPLICABILITY.md) — selected controls.
- [COMPLIANCE-EVIDENCE.md](COMPLIANCE-EVIDENCE.md) — artifact and evidence index.

## Approval

This policy is not approved until the operating organization records a named Accountable Owner and
signs below. A blank signature means the document is a draft for review, not an adopted policy.

| Role | Name | Signature | Date |
|---|---|---|---|
| Accountable Owner | | | |
| AI System Owner | | | |
