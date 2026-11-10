# WP-03 — Operating tests and samples

**Audit ID:** IA-________-01
**Period:** ____________________ to ____________________
**Prepared by:** ______________________________  **Date:** ______________
**Reviewed by:** ______________________________  **Date:** ______________
**Status:** Not started

[Back to workpaper index](README.md)

## 1. Purpose and selection record

This workpaper tests whether the controls operated during the period. It uses the sample sizes in
WP-01 §5. Record the population export, hash and selection seed before filling any result.

| Population | Export file / query | Count | Sample size | Export SHA-256 | Seed / method | Selected IDs attached |
|---|---|---|---|---|---|---|
| Approved applications | | | | | | yes / no |
| Audit events | | | | | | yes / no |
| Overrides | | | | | | yes / no |
| Incidents — critical/high | | | | | | yes / no |
| Incidents — medium/low | | | | | | yes / no |
| Cited claims | | | | | | yes / no |
| Honesty reviews | | | | | | yes / no |
| Material changes | | | | | | yes / no |

**Selection rule reminders**

- Applications: at least 5 or 10%, whichever is greater, capped at 20. Include an override application
  and a false-block application if present.
- Audit events: all if fewer than 50; otherwise 20 random plus every override and every critical
  incident.
- Incidents: all critical/high; 20% or 5 of medium/low, whichever is greater, capped at 10.
- Claims: at least 10 cited claims across sampled applications; include a weak-provenance claim if one
  exists.
- No substitution. A missing selected item is an exception and may be a finding.

## 2. Approved application sample

Add one row per sampled application. Attach the audit event export and the relevant document hashes.

| App ID | Generated (UTC) | Gate verdict | Blocking findings | Override? | Disclosure after latest generation? | Approval event | Model digest present? | Doc hashes match? | Result | Finding |
|---|---|---|---|---|---|---|---|---|---|---|
| | | | | yes / no | yes / no | yes / no | yes / no | yes / no | | |
| | | | | yes / no | yes / no | yes / no | yes / no | yes / no | | |
| | | | | yes / no | yes / no | yes / no | yes / no | yes / no | | |
| | | | | yes / no | yes / no | yes / no | yes / no | yes / no | | |
| | | | | yes / no | yes / no | yes / no | yes / no | yes / no | | |

**Application test steps**

1. Confirm the application was approved in the period.
2. Trace the approved documents to the generation event and the commit-time event.
3. Confirm the gate verdict and each blocking finding are recorded.
4. Confirm a disclosure acknowledgement follows the latest generation.
5. If an override exists, confirm a written reason, timestamp and finding summary are present.
6. Confirm the commit-time model digest and final document hashes are present.
7. Recompute or compare the document hashes where the tooling allows; record any mismatch.

**Application result summary:** tested ____; conforming ____; exceptions ____.

## 3. Claim and provenance sample

Add one row per sampled cited claim. Select at least 10, or all if fewer.

| Claim ID | App ID | Evidence type | Evidence reference | SHA-256 fingerprint | Verifier | Confirmed at | Cited in doc? | `verify_claim_evidence` result | Weak provenance? | Result | Finding |
|---|---|---|---|---|---|---|---|---|---|---|---|
| | | | | | | | yes / no | pass / mismatch / missing | yes / no | | |
| | | | | | | | yes / no | pass / mismatch / missing | yes / no | | |
| | | | | | | | yes / no | pass / mismatch / missing | yes / no | | |
| | | | | | | | yes / no | pass / mismatch / missing | yes / no | | |
| | | | | | | | yes / no | pass / mismatch / missing | yes / no | | |
| | | | | | | | yes / no | pass / mismatch / missing | yes / no | | |
| | | | | | | | yes / no | pass / mismatch / missing | yes / no | | |
| | | | | | | | yes / no | pass / mismatch / missing | yes / no | | |
| | | | | | | | yes / no | pass / mismatch / missing | yes / no | | |
| | | | | | | | yes / no | pass / mismatch / missing | yes / no | | |

**Claim test steps**

1. Confirm the claim is confirmed and is actually cited by the sampled document.
2. Check the evidence type and reference are meaningful, not placeholders.
3. Run or inspect `verify_claim_evidence`; record mismatches.
4. Confirm the verifier and confirmation time are present.
5. Confirm weak provenance is surfaced as an advisory and not described as verified evidence.
6. If the claim is legacy (`unspecified`), record the limitation and whether re-confirmation is
   planned.

**Claim result summary:** tested ____; conforming ____; weak/legacy ____; mismatches ____.

## 4. Audit events, disclosure and override sample

Add one row per sampled event. Use the mandatory additions from WP-01.

| Event ID | App ID | Event type | UTC timestamp | App version | Model digest | Template version | Rule-set hash | Document hashes | Verdict / reason | Result | Finding |
|---|---|---|---|---|---|---|---|---|---|---|---|
| | | generated / disclosure / approved / override | | | | | | | | | |
| | | | | | | | | | | | |
| | | | | | | | | | | | |
| | | | | | | | | | | | |
| | | | | | | | | | | | |

**Event test steps**

1. Confirm the event is append-only in practice: the apply-time report is not overwritten by commit.
2. Confirm the event has an ID, UTC timestamp, app version, application ID and event type.
3. Confirm the model name and digest, template version, rule IDs/hash and document hashes are present
   where the event type requires them.
4. Confirm the commit event records the commit-time digest and final hashes.
5. Confirm disclosure acknowledgement precedes approval and follows the latest generation.
6. Confirm an override event includes the written reason and finding summary.

**Event result summary:** tested ____; conforming ____; missing fields ____.

## 5. Incident and concerns sample

Add every critical/high incident and the sampled medium/low incidents.

| Incident ID | App ID | Category | Severity | Received | Acknowledged within target? | Investigation / root cause | Corrective action | Owner / due | Closed | Appeal recorded? | Result | Finding |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| | | | critical / high / medium / low | | yes / no | | | | yes / no | yes / no / N/A | | |
| | | | | | yes / no | | | | yes / no | yes / no / N/A | | |
| | | | | | yes / no | | | | yes / no | yes / no / N/A | | |
| | | | | | yes / no | | | | yes / no | yes / no / N/A | | |
| | | | | | yes / no | | | | yes / no | yes / no / N/A | | |

**Incident test steps**

1. Confirm the category and severity are recorded with a rationale.
2. Compare the acknowledgement date with the severity target in CONCERNS-CHANNEL §5.
3. Confirm containment, investigation and root cause are recorded.
4. Confirm corrective action has an owner, due date and completion evidence.
5. Confirm the reporter received an outcome and was told about the appeal route.
6. Confirm non-retaliation and confidentiality were stated.
7. For a critical incident, confirm AO escalation and consideration of an impact-assessment review.

**Incident result summary:** tested ____; on time ____; late ____; open ____.

## 6. Monitoring and objectives sample

Use the objectives and the actual metrics source. Record denominators and sample limitations; do not
accept a rate without them.

| Objective | Metric | Numerator | Denominator | Target | Actual | Source | Sample limitation | Result | Finding |
|---|---|---|---|---|---|---|---|---|---|
| H2 | Disclosure completeness | | | 100% | | `disclosures.jsonl`; audit events | | | |
| H3 | Blocking findings resolved/overridden | | | 100% | | audit events | | | |
| H5 | Incidents acknowledged/closed on time | | | 100% / ≥80% | | incident log | | | |
| H6 | False-block rate | | | ≤5% | | incidents + review | | | |
| H8 | Review cadence | | | 100% | | `reviews.jsonl` | | | |
| H1 | Confirmed ungrounded statements in sample | | | 0 | | audit sample | | | |
| H4 | Provenance completeness | | | ≥95% | | claims + verifier | | | |
| H7/H9 | Guardrail suite / advisory-only blocks | | | 100% / 0 | | `cargo test`; code review | | | |
| H10 | Material changes assessed | | | 100% | | change log | | | |

- [ ] Every reported rate has a numerator, denominator and period.
- [ ] A rate with fewer than 5 observations is marked “insufficient sample,” not “met.”
- [ ] “No reports” is recorded as no reports, not zero harm.
- [ ] The 90-day review record includes reviewer, date, note and observed counts.
- [ ] A missed target has a CAPA and a management-review input.

## 7. Change control sample

Add every material change in the period.

| Change ID | Type | Material? | Impact assessment reviewed? | Regression run? | Audit/change note? | Release date | Result | Finding |
|---|---|---|---|---|---|---|---|---|
| | model / rule / template / threshold / scope | yes / no | yes / no / N/A | yes / no | yes / no | | | |
| | | yes / no | yes / no / N/A | yes / no | yes / no | | | |
| | | yes / no | yes / no / N/A | yes / no | yes / no | | | |

- [ ] The change is classified against AI-IMPACT-ASSESSMENT §6.
- [ ] A material change has an impact review before release.
- [ ] The regression run includes the guardrail fixtures.
- [ ] The change record states why a change is non-material, if claimed.
- [ ] No change silently weakens a guardrail.

## 8. Supplier and model identity

| Model name | Digest | Runtime | Hosted inference? | Supplier limitations recorded? | Used in sampled apps? | Result | Finding |
|---|---|---|---|---|---|---|---|
| | | Ollama / other | yes / no | yes / no | yes / no | | |
| | | | yes / no | yes / no | yes / no | | |

- [ ] The digest observed at generation matches the digest recorded in the audit event.
- [ ] A digest change is visible between generation and commit.
- [ ] No document text is sent to a hosted model by Veridicus.
- [ ] Supplier limitations are reflected in the system card.
- [ ] The model tag is not treated as immutable without a digest check.

## 9. Result summary and sign-off

| Result | Count | Finding IDs |
|---|---|---|
| Conforming | | |
| Observation | | |
| Minor nonconformity | | |
| Major nonconformity | | |
| Not tested / limitation | | |

**Operating test conclusion:**

```
______________________________________________________________________________
______________________________________________________________________________
```

| Role | Name | Signature | Date |
|---|---|---|---|
| Preparer | | | |
| Reviewer | | | |
