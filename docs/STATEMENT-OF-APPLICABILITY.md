# Statement of Applicability

**Artifact ID:** C-07
**Version:** 1.0
**Effective date:** 2026-11-07, on approval
**Owner:** Accountable Owner
**Approval:** ______________________________  Date: ______________
**Status:** draft for approval. This is **not** a certification, a declaration of conformity, or a
substitute for an accredited audit. Short control titles are used as anchors; the normative
requirements are in the purchased ISO/IEC 42001:2023 text.

## 1. Purpose and scope

This Statement of Applicability (SoA) records every Annex A control in ISO/IEC 42001:2023, whether it
is applicable to the CareerOS demonstrator, the justification, and the evidence that implements or
supports it. It also records the clause-level evidence for the management-system requirements that
matter most to this demonstrator.

**Scope of the AI management system:** the local, single-user CareerOS Desktop application, its
honesty guardrails, local model use, claims ledger, generated documents, audit record, disclosure and
incident flow.

**Out of scope:** model training or fine-tuning, hosted or multi-tenant inference, autonomous
application submission, employment screening or selection decisions, and legal or immigration advice.

## 2. Method

1. The impact assessment in [AI-IMPACT-ASSESSMENT.md](AI-IMPACT-ASSESSMENT.md) identifies affected
   groups, impacts and review triggers.
2. Each Annex A control was assessed against that scope. A control is applicable if the system or the
   operating organization does, or could, affect the control objective.
3. Applicable controls are selected and mapped to an artifact or code path. Controls that do not apply
   are excluded with a reason and a re-evaluation trigger.
4. Status is recorded honestly: `Implemented` means the control operates in the repository;
   `Documented` means the artifact exists but organizational adoption, approval or operation is still
   required; `Partial` means a known gap remains; `Not applicable` means the control is excluded.
5. The SoA is reviewed with the impact assessment and management review, and after a material change.

## 3. Applicability summary

- Annex A controls considered: **38**.
- Selected: **35**.
- Excluded: **3** — A.7.2, A.8.5 and A.10.4.
- No control is excluded merely because it is inconvenient. Each exclusion has a scope reason and a
  trigger to revisit it.

## 4. Annex A controls

| Control | Short title | Applicable | Justification | Status | Primary evidence |
|---|---|---|---|---|---|
| A.2.2 | AI policy | Yes | The organization needs a policy for truthful generation, human oversight and prohibited uses. | Documented | [AI-POLICY.md](AI-POLICY.md) |
| A.2.3 | Alignment with other organisational policies | Yes | AI policy must align with privacy, security, data and host-organization policies. | Partial | AI-POLICY §3(13), §7; system card data section; host policies still to be attached |
| A.2.4 | Review of the AI policy | Yes | Policy must be reviewed on a stated cycle and on triggers. | Documented | AI-POLICY §9; management review |
| A.3.2 | AI roles and responsibilities | Yes | Accountability, delivery, review and user duties must be assigned. | Documented | [AI-RACI.md](AI-RACI.md) |
| A.3.3 | Reporting of concerns | Yes | Users and affected parties need a protected route to report bad outputs and governance concerns. | Documented | [CONCERNS-CHANNEL.md](CONCERNS-CHANNEL.md); incident log |
| A.4.2 | Resource documentation | Yes | The system's data, tooling, compute, human and supplier resources must be documented. | Partial | AI-SYSTEM-CARD; resource inventory still incomplete |
| A.4.3 | Data resources | Yes | Claims, resumes, postings and application records are data resources with retention rules. | Implemented | AI-SYSTEM-CARD data section; claims ledger; encrypted DB |
| A.4.4 | Tooling resources | Yes | Local model runtime, desktop shell, database and build tools affect system behavior. | Partial | AI-SYSTEM-CARD; no complete versioned tooling inventory |
| A.4.5 | System and computing resources | Yes | Inference runs on the user's local machine; compute limits affect quality and availability. | Partial | AI-SYSTEM-CARD; no capacity or hardware baseline |
| A.4.6 | Human resources | Yes | The operator and reviewer need defined competence and independence. | Partial | AI-RACI; competence expectations in AI-POLICY §8; no training records |
| A.5.2 | AI system impact assessment process | Yes | A repeatable process is required to identify impacts and treatments. | Documented | AI-IMPACT-ASSESSMENT §2–§5 |
| A.5.3 | Documentation of AI system impact assessments | Yes | Assessments and their revisions must be retained. | Documented | AI-IMPACT-ASSESSMENT; version and sign-off block |
| A.5.4 | Assessing AI system impact on individuals or groups | Yes | Job seekers, employers and named colleagues can be harmed by a false draft. | Documented | AI-IMPACT-ASSESSMENT affected groups and risks |
| A.5.5 | Assessing societal impacts of AI systems | Yes | Misrepresentation at scale can affect labor-market trust and fairness. | Documented | AI-IMPACT-ASSESSMENT societal impacts |
| A.6.1.2 | Objectives for responsible development of AI system | Yes | Development needs measurable honesty and human-control objectives. | Documented | [HONESTY-OBJECTIVES.md](HONESTY-OBJECTIVES.md) |
| A.6.1.3 | Processes for responsible design and development of AI systems | Yes | Guardrail design, review and regression testing must be defined. | Implemented | [GUARDRAIL-METHOD.md](GUARDRAIL-METHOD.md); evaluation tests |
| A.6.2.2 | AI system requirements and specification | Yes | Truthfulness, provenance, disclosure and non-submission are system requirements. | Implemented | AI-SYSTEM-CARD; AI-POLICY; guardrail tests |
| A.6.2.3 | Documentation of AI system design and development | Yes | Design decisions and limits must be understandable to an auditor. | Partial | AI-SYSTEM-CARD; GUARDRAIL-METHOD; no complete architecture record |
| A.6.2.4 | AI system verification and validation | Yes | Guardrails need adversarial and regression evidence. | Implemented | `eval/guardrails.json`; `cargo test`; 10-case harness |
| A.6.2.5 | AI system deployment | Yes | Release and local installation can change behavior or evidence. | Partial | Local build; no formal release checklist or deployment record |
| A.6.2.6 | AI system operation and monitoring | Yes | Blocks, overrides, incidents and reviews must be monitored. | Implemented | `/honesty-stats`; Settings honesty panel; 90-day review record |
| A.6.2.7 | AI system technical documentation | Yes | Users and auditors need a technical description and limits. | Implemented | AI-SYSTEM-CARD; GUARDRAIL-METHOD; this SoA |
| A.6.2.8 | AI system recording of event logs | Yes | Generation and approval decisions must be reconstructable. | Implemented | `applications/<id>/audit/events.jsonl`; model/rule/document hashes |
| A.7.2 | Data for development and enhancement of AI system | No | CareerOS does not train, fine-tune or enhance a model; it uses a third-party local model at runtime. | Not applicable | Revisit if training, fine-tuning or a feedback-trained model is added |
| A.7.3 | Acquisition of data | Yes | Claims, resumes and postings are acquired from the user or the web and need lawful, authorized handling. | Implemented | Setup wizard; resume import; claims confirmation; system card |
| A.7.4 | Quality of data for AI systems | Yes | False, stale or weakly evidenced claims undermine every output. | Partial | Provenance fields; weak-provenance advisory; legacy claims still `unspecified` |
| A.7.5 | Data provenance | Yes | Claims must trace to evidence, verifier and confirmation time. | Implemented | `evidence_type`; reference; SHA-256 fingerprint; `verified_by`; `verify_claim_evidence` |
| A.7.6 | Data preparation | Yes | Resume and posting text are parsed, normalized and delimited before generation. | Partial | Import and JD handling; no documented data-preparation specification |
| A.8.2 | System documentation and information for users | Yes | Users need intended use, limits, disclosure and recourse information. | Implemented | AI-SYSTEM-CARD; first-run and per-application disclosure |
| A.8.3 | External reporting | Yes | Serious output, privacy or safety concerns need a route beyond the user. | Documented | CONCERNS-CHANNEL §3, §6; external notification decision reserved to owner |
| A.8.4 | Communication of incidents | Yes | Reporters need acknowledgement, outcome and appeal information. | Documented | CONCERNS-CHANNEL §5–§7; incident status |
| A.8.5 | Information for interested parties | No | The demonstrator is a direct-to-user desktop tool with no external customers, regulators or distribution channel. | Not applicable | Revisit on distribution, hosted service or employer-facing use |
| A.9.2 | Processes for responsible use of AI systems | Yes | Users need clear rules for review, disclosure, override and reporting. | Documented | AI-POLICY; AI-SYSTEM-CARD; review dialog |
| A.9.3 | Objectives for responsible use of AI system | Yes | Responsible-use performance needs measurable targets. | Documented | HONESTY-OBJECTIVES |
| A.9.4 | Intended use of the AI system | Yes | Intended and prohibited uses must be stated and acknowledged. | Implemented | AI-POLICY §4–§5; AI-SYSTEM-CARD; first-run disclosure |
| A.10.2 | Allocation of responsibilities | Yes | The organization remains accountable across the model/tool supply chain. | Documented | AI-RACI; supplier row |
| A.10.3 | Suppliers | Yes | The local model runtime and model artifacts are third-party dependencies. | Partial | Model name/digest in audit; supplier limitations in system card; no supplier model card |
| A.10.4 | Customers | No | There are no customers; the operator is the user in the demonstrator. | Not applicable | Revisit before distribution or commercial use |

## 5. Clause-level evidence

| Clause | Requirement | Evidence | Status |
|---|---|---|---|
| 5.2 | AI policy | AI-POLICY, approval block | Documented; approval pending |
| 5.3 | Roles, responsibilities and authorities | AI-RACI, appointment record | Documented; names pending |
| 6.1.4 / 8.4 | AI system impact assessment | AI-IMPACT-ASSESSMENT, review triggers | Documented; sign-off pending |
| 6.2 | AI objectives and planning | HONESTY-OBJECTIVES, denominators and cadence | Documented; baseline pending |
| 7.1 | Resources | AI-SYSTEM-CARD; AI-RACI | Partial; no resource plan |
| 7.2 | Competence | AI-POLICY §8; AI-RACI | Partial; no training records |
| 7.3 | Awareness | Policy, RACI and disclosure | Partial; no awareness record |
| 7.4 | Communication | CONCERNS-CHANNEL; disclosure flow | Documented |
| 7.5 | Documented information | This SoA; evidence index; audit records | Documented; local, unsigned records |
| 8.1 | Operational planning and control | GUARDRAIL-METHOD; commit gate; release tests | Partial; no formal release checklist |
| 9.1 | Monitoring, measurement, analysis and evaluation | HONESTY-OBJECTIVES; honesty stats; reviews | Partial; metrics defined, baseline not measured |
| 9.2 | Internal audit | INTERNAL-AUDIT | Documented; **not performed** |
| 9.3 | Management review | MANAGEMENT-REVIEW | Documented; **not held** |
| 10.1 | Continual improvement | Objectives; CAPA in CONCERNS-CHANNEL; management review | Documented |
| 10.2 | Nonconformity and corrective action | INTERNAL-AUDIT §7; CONCERNS-CHANNEL §5, §7 | Documented; no CAPA history yet |

## 6. Exclusions

| Control | Reason for exclusion | Re-evaluation trigger |
|---|---|---|
| A.7.2 Data for development and enhancement | No model training, fine-tuning, evaluation-data curation or enhancement loop. The model is third-party and used as supplied. | Any training, fine-tuning, feedback-trained model, or model evaluation dataset. |
| A.8.5 Information for interested parties | No external distribution, hosted service, employer-facing interface or regulator relationship in the demonstrator. | Distribution to other users, hosted deployment, commercial use or employer-facing features. |
| A.10.4 Customers | No customers; the operator is the sole user. | Any customer, client, employer or third-party user. |

## 7. Known gaps and actions

| Gap | Action | Owner | Target |
|---|---|---|---|
| Policy, RACI, objectives and SoA are unapproved drafts | Name role holders; sign the approval blocks; record the adoption date | AO | Before claiming an operating AIMS |
| No independent internal audit | Appoint an Independent Reviewer and complete the initial audit | AO | Date to be set |
| No management review held | Hold the first review with objectives, incidents and audit input | AO | Date to be set |
| No baseline for honesty objectives | Measure the first period; record denominators and sample sizes | SO | First quarterly review |
| No complete tooling/resource inventory | Add versions and support status for the runtime, model, database and build tools | SO | Next system-card review |
| No release/deployment checklist | Define release checks and retain the test output with the release | SO | Next release |
| No structured supplier model card | Request or record model limitations and intended use from the supplier | SO | Next model change |
| Rule changes have no history table | Preserve prior rule text or a versioned rules file alongside the hash | SO | Next rule change |
| No live Ollama exercise of disclosure, incident and review | Run one end-to-end `/apply` and record the evidence | MO | Before audit |

## 8. Approval

This SoA is not approved until the Accountable Owner signs it. Approval records the applicability
decisions and the known gaps; it does not certify the system.

| Role | Name | Signature | Date |
|---|---|---|---|
| Accountable Owner | | | |
| AI System Owner | | | |
| Independent Reviewer (reviewed) | | | |

## 9. Review

Review this SoA at least annually and whenever the impact assessment, scope, model, rule set,
template, distribution model or legal obligations change. A control may be added, removed or
reclassified only with a recorded rationale; do not silently drop a control to improve the appearance
of conformity.
