# Measurable honesty objectives

**Artifact ID:** C-04
**Version:** 1.0
**Effective date:** 2026-11-07, on approval
**Owner:** AI System Owner
**Approval:** ______________________________  Date: ______________
**Status:** draft for adoption. Targets become commitments only when the Accountable Owner approves
them. No baseline has been recorded yet; the first measurement period establishes it.

## 1. Purpose

This document sets measurable objectives for responsible AI and honesty, supporting ISO/IEC 42001
clause 6.2 and A.9.3. The objectives are deliberately narrow: they measure whether CareerOS keeps
generated materials grounded, keeps humans in control, and learns from failures.

They are not marketing metrics. A target is met only when the stated evidence exists; absence of
reports is not evidence of zero harm.

## 2. Objectives

| ID | Objective | Metric and denominator | Target | Evidence source | Frequency | Owner |
|---|---|---|---|---|---|---|
| **H1** | No confirmed ungrounded factual statement in approved materials | Confirmed ungrounded statements ÷ factual statements reviewed in the quarterly sample | **0** | Independent review of a sample of approved applications against confirmed claims | Quarterly | SO |
| **H2** | Every approval is preceded by disclosure | Approved applications with a valid per-application acknowledgement ÷ approved applications | **100%** | `compliance/disclosures.jsonl`; per-application `disclosure_acknowledged` audit events | Monthly | MO |
| **H3** | Every blocking finding is resolved or explicitly overridden | Blocking findings with a recorded removal, correction or written override ÷ blocking findings | **100%** | Application audit events; override records | Monthly | MO |
| **H4** | Claims carry usable provenance | Cited claims with evidence type, reference and verifier ÷ cited claims | **≥95% by 2027-11-07** | Claims ledger; `verify_claim_evidence`; weak-provenance reports | Quarterly | MO |
| **H5** | Concerns are acknowledged and resolved on time | Reports acknowledged within the severity target ÷ reports; reports closed within target ÷ reports | **100% acknowledged; ≥80% closed** | Incident log / CSV; concerns register | Monthly | SO |
| **H6** | False blocks are measured, not hidden | Confirmed false-block reports ÷ blocking findings | **≤5%**, with at least 10 blocking findings in the period | Incident records; review confirmation | Quarterly | SO |
| **H7** | The guardrail suite passes on every release | Passing guardrail/evaluation tests ÷ required tests | **100%**, no skipped required case | `cargo test`; evaluation fixture results | Per release | MO |
| **H8** | Honesty review happens on cadence | Reviews recorded ÷ expected 90-day reviews | **100%**, at least 4 per year | `compliance/reviews.jsonl` | Quarterly | SO |
| **H9** | Advisory findings never act as blocks | Blocking findings attributable only to the advisory scope review | **0** | Guardrail tests; code review of findings | Per release | MO |
| **H10** | Material changes are assessed before release | Material model/rule/template changes with impact review and regression record ÷ material changes | **100%** | Change log; impact-assessment revision; test record | Per change | SO |
| **H11** | Injection attempts are recorded, not silently followed | Job postings with detected instruction-like text that produce a recorded warning ÷ detected postings | **100%** | Audit findings; JD ingestion tests | Per release / monthly | MO |

## 3. Definitions and measurement rules

- **Approved application:** an application whose documents passed the gate and were approved by the
  user, including an override.
- **Blocking finding:** a deterministic finding with severity `block`; an advisory finding is not a
  block.
- **Confirmed ungrounded statement:** a factual statement that is not supported by a confirmed claim,
  confirmed by the reviewer after checking the ledger. A stylistic difference is not a finding.
- **False block:** a user-reported block that the reviewer confirms was not required by the claim
  ledger or the stated rules. A user disagreement alone is not a confirmed false block.
- **Cited claim:** a claim referenced by a generated document through the claim ID / provenance path.
- **Material change:** a change that can alter which statements are generated or blocked, including a
  model family or digest, prompt contract, rule, threshold, template version or guardrail dependency.

Rules:

1. Report the numerator, denominator and sample size. A rate with fewer than 5 observations is marked
   **insufficient sample**, not “met.”
2. Do not count “no reports received” as zero incidents. Report it as “no reports” and state the
   channel’s reach.
3. Do not pool periods to hide a miss. Show the period and the trend.
4. A target miss is a nonconformity candidate and enters corrective action (§5).
5. Metrics are computed from the records named above; a manually typed number without a source is not
   evidence.

## 4. Reporting cadence

| Activity | Frequency | Record |
|---|---|---|
| Operational metrics (H2, H3, H5, H11) | Monthly | Honesty metrics summary in the workspace |
| Honesty review (H1, H4, H6, H8) | Every 90 days | `compliance/reviews.jsonl`, with reviewer and counts |
| Release checks (H7, H9) | Every release | Test output attached to the release record |
| Change checks (H10) | Every material change | Impact-assessment revision and regression record |
| Objective performance review | At least annually, with management review | [MANAGEMENT-REVIEW.md](MANAGEMENT-REVIEW.md) minutes |

The 90-day honesty review is an operational review, not a management review. It supplies input to
clause 9.3.

## 5. When a target is missed

1. Record the miss, the period and the evidence within 10 business days of detection.
2. Identify the root cause (rule gap, template, model behavior, data quality, user workflow, channel
   reach or measurement defect).
3. Open a corrective action with an owner, due date and success measure; record it in the concerns /
   CAPA register.
4. Decide whether the impact assessment, Statement of Applicability or policy needs revision.
5. Report the miss and action at the next management review; verify effectiveness before closing.

## 6. Review

Review the objectives annually and after a serious incident, a material change, or two consecutive
misses. Retire or replace an objective only with a recorded rationale; do not lower a target solely to
make performance look better.
