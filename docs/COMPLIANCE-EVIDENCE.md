# Compliance evidence index — guardrails and honesty

**Frameworks:** NIST AI RMF 1.0 and NIST AI 600-1 (Generative AI Profile); ISO/IEC 42001:2023, with
ISO/IEC 23894:2023 and ISO/IEC 42005:2025 as guidance.
**Status:** alignment evidence, not certification. ISO/IEC 42001 certifies an organization's AI
management system; NIST AI RMF is voluntary. Do not describe this application as certified or
compliant.

This index maps each selected control to the artifact or code path that evidences it. The full
applicability decision, including the three exclusions, is in
[STATEMENT-OF-APPLICABILITY.md](STATEMENT-OF-APPLICABILITY.md).

## 1. Artifact register

| Artifact | Purpose | Owner | Status |
|---|---|---|---|
| [AI-POLICY.md](AI-POLICY.md) | Policy, intended/prohibited use, human oversight, review triggers | AO | Draft; approval pending |
| [AI-RACI.md](AI-RACI.md) | Roles, decision rights, independence limitation | AO | Draft; names pending |
| [CONCERNS-CHANNEL.md](CONCERNS-CHANNEL.md) | Intake, triage, confidentiality, non-retaliation, appeal | SO | Draft; contact pending |
| [AI-IMPACT-ASSESSMENT.md](AI-IMPACT-ASSESSMENT.md) | Affected groups, impacts, treatments, change triggers | SO | Draft; sign-off pending |
| [HONESTY-OBJECTIVES.md](HONESTY-OBJECTIVES.md) | Measurable objectives, denominators, cadence, CAPA | SO | Draft; baseline pending |
| [INTERNAL-AUDIT.md](INTERNAL-AUDIT.md) | Audit programme, method, sampling, findings, CAPA | AO/IR | Procedure only; not performed |
| [audit/README.md](audit/README.md) | First audit workpaper pack: plan, sampling, tests, findings, sign-off | IR | Blank; audit not started |
| [audit/EVIDENCE-REQUEST.md](audit/EVIDENCE-REQUEST.md) | Auditor evidence request, acceptance criteria and receipt | IR | Template; not issued |
| [MANAGEMENT-REVIEW.md](MANAGEMENT-REVIEW.md) | Review inputs, decisions, minutes, follow-up | AO | Procedure only; not held |
| [STATEMENT-OF-APPLICABILITY.md](STATEMENT-OF-APPLICABILITY.md) | All 38 Annex A controls, applicability and evidence | AO | Draft; approval pending |
| [OPERATOR-ADOPTION-CHECKLIST.md](OPERATOR-ADOPTION-CHECKLIST.md) | Consolidated role, contact, review and signature record | AO | Blank; to be completed |
| [AI-SYSTEM-CARD.md](AI-SYSTEM-CARD.md) | System purpose, behavior, limits, disclosure | SO | Current |
| [GUARDRAIL-METHOD.md](GUARDRAIL-METHOD.md) | Deterministic checks, advisory scope, commit gate, audit record | SO | Current |

## 2. Clause-level evidence

| Clause | Evidence | Status |
|---|---|---|
| 5.2 AI policy | AI-POLICY | Documented; approval pending |
| 5.3 Roles and authorities | AI-RACI; appointment record | Documented; names pending |
| 6.1.4 / 8.4 Impact assessment | AI-IMPACT-ASSESSMENT §2–§6 | Documented; sign-off pending |
| 6.2 Objectives | HONESTY-OBJECTIVES | Documented; baseline pending |
| 7.1 Resources | AI-SYSTEM-CARD; AI-RACI | Partial |
| 7.2 Competence | AI-POLICY §8; AI-RACI | Partial |
| 7.3 Awareness | Policy, RACI, disclosure | Partial |
| 7.4 Communication | CONCERNS-CHANNEL; disclosure flow | Documented |
| 7.5 Documented information | This index; SoA; audit records | Documented; local, unsigned |
| 8.1 Operational control | GUARDRAIL-METHOD; commit gate; tests | Partial; no release checklist |
| 9.1 Monitoring and measurement | HONESTY-OBJECTIVES; `/honesty-stats`; reviews | Partial; baseline pending |
| 9.2 Internal audit | INTERNAL-AUDIT; audit/README.md; audit/EVIDENCE-REQUEST.md | Procedure, blank workpapers and request package; **not performed** |
| 9.3 Management review | MANAGEMENT-REVIEW | Procedure only; **not held** |
| 10.1 Continual improvement | Objectives; CAPA; management review | Documented |
| 10.2 Nonconformity and corrective action | INTERNAL-AUDIT §7; CONCERNS-CHANNEL §5, §7 | Documented; no CAPA history |

## 3. Selected Annex A controls

Status legend: **Implemented** = operates in the repository; **Documented** = artifact exists, but
adoption/approval/operation is pending; **Partial** = a known gap remains.

| Control | Short title | Evidence | Status |
|---|---|---|---|
| A.2.2 | AI policy | AI-POLICY | Documented |
| A.2.3 | Alignment with other policies | AI-POLICY §2, §7; host policies to attach | Partial |
| A.2.4 | Review of the AI policy | AI-POLICY §9; MANAGEMENT-REVIEW | Documented |
| A.3.2 | AI roles and responsibilities | AI-RACI | Documented |
| A.3.3 | Reporting of concerns | CONCERNS-CHANNEL; incident log | Documented |
| A.4.2 | Resource documentation | AI-SYSTEM-CARD; AI-RACI | Partial |
| A.4.3 | Data resources | AI-SYSTEM-CARD data section; claims ledger; encrypted DB | Implemented |
| A.4.4 | Tooling resources | AI-SYSTEM-CARD; tooling inventory incomplete | Partial |
| A.4.5 | System and computing resources | AI-SYSTEM-CARD; no capacity baseline | Partial |
| A.4.6 | Human resources | AI-RACI; AI-POLICY §8 | Partial |
| A.5.2 | Impact assessment process | AI-IMPACT-ASSESSMENT §2–§6 | Documented |
| A.5.3 | Documentation of impact assessments | AI-IMPACT-ASSESSMENT; version and sign-off | Documented |
| A.5.4 | Impacts on individuals or groups | AI-IMPACT-ASSESSMENT §3–§4 | Documented |
| A.5.5 | Societal impacts | AI-IMPACT-ASSESSMENT §3–§4 | Documented |
| A.6.1.2 | Responsible-development objectives | HONESTY-OBJECTIVES | Documented |
| A.6.1.3 | Responsible design and development | GUARDRAIL-METHOD; evaluation tests | Implemented |
| A.6.2.2 | Requirements and specification | AI-SYSTEM-CARD; AI-POLICY; guardrail tests | Implemented |
| A.6.2.3 | Design and development documentation | AI-SYSTEM-CARD; GUARDRAIL-METHOD; no complete architecture record | Partial |
| A.6.2.4 | Verification and validation | `eval/guardrails.json`; `cargo test`; 10-case harness | Implemented |
| A.6.2.5 | Deployment | Local build; no formal release checklist | Partial |
| A.6.2.6 | Operation and monitoring | `/honesty-stats`; Settings honesty panel; reviews | Implemented |
| A.6.2.7 | Technical documentation | AI-SYSTEM-CARD; GUARDRAIL-METHOD; SoA | Implemented |
| A.6.2.8 | Event logs | `applications/<id>/audit/events.jsonl`; model/rule/document hashes | Implemented |
| A.7.3 | Acquisition of data | Setup; resume import; claims confirmation; system card | Implemented |
| A.7.4 | Quality of data | Provenance fields; weak-provenance advisory | Partial; legacy claims |
| A.7.5 | Data provenance | SHA-256 fingerprint; verifier; `verify_claim_evidence` | Implemented |
| A.7.6 | Data preparation | Import and JD handling; no documented specification | Partial |
| A.8.2 | Information for users | AI-SYSTEM-CARD; first-run and per-application disclosure | Implemented |
| A.8.3 | External reporting | CONCERNS-CHANNEL §3, §6 | Documented |
| A.8.4 | Communication of incidents | CONCERNS-CHANNEL §5–§7 | Documented |
| A.9.2 | Responsible-use processes | AI-POLICY; AI-SYSTEM-CARD; review dialog | Documented |
| A.9.3 | Responsible-use objectives | HONESTY-OBJECTIVES | Documented |
| A.9.4 | Intended use | AI-POLICY §4–§5; AI-SYSTEM-CARD; disclosure | Implemented |
| A.10.2 | Allocation of responsibilities | AI-RACI | Documented |
| A.10.3 | Suppliers | Model name/digest; supplier limitations | Partial |

**Excluded controls:** A.7.2 (no model training or enhancement), A.8.5 (no external distribution),
A.10.4 (no customers). Reasons and re-evaluation triggers are in the SoA §6.

## 4. How to produce evidence for a run

1. Run `/apply` for a posting.
2. Open the application folder and inspect `audit/events.jsonl`.
   The `generated` event contains the model digest, template version, rule ids and hash, document
   hashes, findings and the blocking verdict.
3. Acknowledge the per-application AI-assisted disclosure in the review dialog. A
   `disclosure_acknowledged` event is appended.
4. Approve in the review dialog. A second event records `approved` or `override`, with the
   commit-time model digest and the final document hashes.
5. Run `/honesty-stats`, or open Settings, for aggregate counts and the review due date.
6. Record a review with the reviewer, note and observed counts; it is appended to
   `compliance/reviews.jsonl` and resets the 90-day cadence.
7. Export incidents from the review dialog as CSV when a problem is reported.
8. Run `cargo test` to repeat the fixture evaluation.
9. For an audit, sample approved applications, incidents, overrides and claims as described in
   INTERNAL-AUDIT §5; record the sample frame and limitations.

## 5. Evidence integrity notes

- Audit events are append-only. The commit report is written to
  `guardrail-report-commit.json`; it does not overwrite the apply-time report.
- `claims_used` records only claims cited in the documents, not every confirmed claim.
- `materials_version` is a content hash of the rendered documents, not a fixed label.
- The Git commit in the workspace is a second, versioned snapshot of approved materials.
- The audit record is local and not signed. A user with file access can alter it; Git history makes
  casual alteration visible but does not make the record tamper-proof.
- The evidence index is a map, not proof that the control operated. Operation is tested by sampling
  and audit.

## 6. Known gaps

- Policy, RACI, objectives, impact assessment and SoA are drafts with blank approvals; they are not
  adopted organizational commitments until the operating organization completes
  OPERATOR-ADOPTION-CHECKLIST and signs the source artifacts.
- No independent internal audit and no management review have been performed. The procedures exist;
  the evidence of operation does not.
- No baseline has been measured for the honesty objectives; all targets are prospective.
- Claims created before P1 default to `evidence_type = unspecified` and no verifier; they surface as
  weak provenance until re-confirmed.
- No rule-change history table; the rule hash detects change but not the prior text.
- No complete tooling/resource inventory, release checklist or supplier model card.
- The 90-day review is local and unsigned; it demonstrates a cadence, not independent audit.
- The advisory model pass and the disclosure/incident/review flow have not been exercised against a
  live Ollama instance in automated tests.
- The concerns channel is local or operator-supplied; there is no independently operated hotline in
  the repository.
- The app cannot retract a submitted application. Recourse after submission depends on the employer,
  the operator's process and applicable law.
