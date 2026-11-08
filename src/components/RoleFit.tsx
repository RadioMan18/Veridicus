import { CheckCircle2, HelpCircle, MinusCircle, XCircle } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { pushToast } from './Toaster';

export interface KeywordCoverage {
  term: string;
  in_jd: boolean;
  in_resume: boolean;
  in_claims: boolean;
}

export interface RoleFitGap {
  requirement: string;
  requirement_tier: string;
  matched_claim_id: string | null;
  adjacent_strength: string;
  proposed_letter_line: string;
  cv_focus?: string;
  cover_letter_point?: string;
  story_ids?: string[];
}

export interface RoleFitAnalysis {
  supported_claim_ids: string[];
  identified_gaps: RoleFitGap[];
  ask_first_triggered: string[];
  ambiguous_requirements: string[];
  hiring_goals?: string[];
  standout_traits?: string[];
  ats_keywords?: KeywordCoverage[];
}

export interface PersistedGap {
  id: string;
  requirement: string;
  requirement_tier: string;
  adjacent_strength: string;
  letter_line: string | null;
  cv_focus: string;
  cover_letter_point: string;
  story_ids: string[];
  status: string;
}

function tierLabel(tier: string) {
  if (tier === 'must_have') return 'Must have';
  if (tier === 'gate') return 'Gate';
  if (tier === 'nice_to_have') return 'Nice to have';
  return tier;
}

function tierStyle(tier: string) {
  if (tier === 'nice_to_have') return 'border-aether-border text-aether-text-muted';
  return 'border-aether-warning text-aether-warning';
}

function CoverageMark({ covered }: { covered: boolean }) {
  return covered ? (
    <CheckCircle2 size={14} className="text-aether-success" aria-label="covered" />
  ) : (
    <XCircle size={14} className="text-aether-text-muted" aria-label="not covered" />
  );
}

export default function RoleFit({
  analysis,
  gaps,
  onDecision,
}: {
  analysis: RoleFitAnalysis | null;
  gaps: PersistedGap[];
  onDecision?: () => void;
}) {
  const decide = async (id: string, status: string) => {
    try {
      await invoke('set_gap_status', { gapId: id, status });
      onDecision?.();
    } catch (error: any) {
      pushToast({
        kind: 'error',
        message: 'Could not record that decision',
        detail: error?.message || String(error),
      });
    }
  };

  const hasAnything = Boolean(analysis) || gaps.length > 0;

  if (!hasAnything) {
    return (
      <p className="text-body text-aether-text-muted">
        No role-fit analysis yet. Run /apply on a posting to build one.
      </p>
    );
  }

  const goals = analysis?.hiring_goals ?? [];
  const traits = analysis?.standout_traits ?? [];
  const keywords = analysis?.ats_keywords ?? [];
  const strengths = analysis?.supported_claim_ids ?? [];
  const ambiguous = analysis?.ambiguous_requirements ?? [];
  const askFirst = analysis?.ask_first_triggered ?? [];

  return (
    <div className="space-y-5">
      {(goals.length > 0 || traits.length > 0) && (
        <div className="grid gap-3 sm:grid-cols-2">
          <section className="rounded-lg border border-aether-border bg-aether-bg p-3">
            <h4 className="mb-2 text-meta font-semibold uppercase tracking-wider text-aether-text-muted">
              What this hire is for
            </h4>
            {goals.length === 0 ? (
              <p className="text-meta text-aether-text-muted">Not stated in the posting.</p>
            ) : (
              <ul className="list-disc space-y-1 pl-4 text-meta text-aether-text">
                {goals.map((goal) => (
                  <li key={goal}>{goal}</li>
                ))}
              </ul>
            )}
          </section>
          <section className="rounded-lg border border-aether-border bg-aether-bg p-3">
            <h4 className="mb-2 text-meta font-semibold uppercase tracking-wider text-aether-text-muted">
              Standout traits
            </h4>
            {traits.length === 0 ? (
              <p className="text-meta text-aether-text-muted">Not stated in the posting.</p>
            ) : (
              <ul className="list-disc space-y-1 pl-4 text-meta text-aether-text">
                {traits.map((trait) => (
                  <li key={trait}>{trait}</li>
                ))}
              </ul>
            )}
          </section>
        </div>
      )}

      <section>
        <h4 className="mb-2 text-meta font-semibold uppercase tracking-wider text-aether-text-muted">
          Strengths ({strengths.length})
        </h4>
        {strengths.length === 0 ? (
          <p className="text-meta text-aether-text-muted">
            No confirmed claim matched a requirement in this posting.
          </p>
        ) : (
          <ul className="flex flex-wrap gap-1.5">
            {strengths.map((claimId) => (
              <li
                key={claimId}
                className="rounded border border-aether-success/50 bg-aether-bg px-2 py-0.5 font-mono text-meta text-aether-success"
                title={`Confirmed claim ${claimId}`}
              >
                {claimId.slice(0, 8)}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section>
        <h4 className="mb-2 text-meta font-semibold uppercase tracking-wider text-aether-text-muted">
          Gaps ({gaps.length})
        </h4>
        {gaps.length === 0 ? (
          <p className="text-meta text-aether-success">Every requirement maps to a confirmed claim.</p>
        ) : (
          <ul className="space-y-2">
            {gaps.map((gap) => (
              <li key={gap.id} className="rounded-lg border border-aether-border bg-aether-bg p-3">
                <div className="flex flex-wrap items-start justify-between gap-2">
                  <p className="min-w-0 flex-1 text-body text-aether-text">{gap.requirement}</p>
                  <span
                    className={`shrink-0 rounded-full border px-2 py-0.5 text-meta ${tierStyle(
                      gap.requirement_tier,
                    )}`}
                  >
                    {tierLabel(gap.requirement_tier)}
                  </span>
                </div>

                <div className="mt-2 space-y-1 text-meta text-aether-text-muted">
                  <p>
                    <span className="font-medium text-aether-text">CV: </span>
                    {gap.cv_focus || 'Show the closest evidence you have.'}
                  </p>
                  <p>
                    <span className="font-medium text-aether-text">Letter: </span>
                    {gap.cover_letter_point || gap.adjacent_strength}
                  </p>
                  {gap.story_ids.length === 0 && (
                    <p className="flex items-center gap-1">
                      <HelpCircle size={12} /> no story yet
                    </p>
                  )}
                </div>

                <div className="mt-2 flex flex-wrap items-center gap-2">
                  <span className="text-meta text-aether-text-muted">
                    Status: <span className="font-mono">{gap.status}</span>
                  </span>
                  {gap.status === 'accepted_by_user' ? (
                    <button
                      onClick={() => void decide(gap.id, 'open')}
                      className="flex items-center gap-1 rounded border border-aether-border px-2 py-1 text-meta text-aether-text transition hover:bg-aether-surface"
                    >
                      <MinusCircle size={12} /> Keep in the letter
                    </button>
                  ) : (
                    <button
                      onClick={() => void decide(gap.id, 'accepted_by_user')}
                      className="flex items-center gap-1 rounded border border-aether-border px-2 py-1 text-meta text-aether-text-muted transition hover:bg-aether-surface"
                      title="The letter will not have to name it; the guardrail report records the omission."
                    >
                      <MinusCircle size={12} /> Leave unaddressed
                    </button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      {askFirst.length > 0 && (
        <section>
          <h4 className="mb-2 text-meta font-semibold uppercase tracking-wider text-aether-text-muted">
            Ask first ({askFirst.length})
          </h4>
          <ul className="space-y-1 text-meta text-aether-warning">
            {askFirst.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </section>
      )}

      {ambiguous.length > 0 && (
        <section>
          <h4 className="mb-2 text-meta font-semibold uppercase tracking-wider text-aether-text-muted">
            Ambiguous ({ambiguous.length})
          </h4>
          <ul className="space-y-1 text-meta text-aether-text-muted">
            {ambiguous.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </section>
      )}

      <section>
        <h4 className="mb-2 text-meta font-semibold uppercase tracking-wider text-aether-text-muted">
          Keyword coverage ({keywords.filter((entry) => entry.in_resume).length}/{keywords.length})
        </h4>
        {keywords.length === 0 ? (
          <p className="text-meta text-aether-text-muted">No posting terms were extracted.</p>
        ) : (
          <div className="max-h-64 overflow-y-auto rounded-lg border border-aether-border">
            <table className="w-full text-meta">
              <thead className="bg-aether-surface text-aether-text-muted">
                <tr>
                  <th className="px-2 py-1 text-left font-medium">Term</th>
                  <th className="px-2 py-1 text-center font-medium">Resume</th>
                  <th className="px-2 py-1 text-center font-medium">Claims</th>
                </tr>
              </thead>
              <tbody>
                {keywords.map((entry) => (
                  <tr key={entry.term} className="border-t border-aether-border">
                    <td className="px-2 py-1 text-aether-text">{entry.term}</td>
                    <td className="px-2 py-1 text-center">
                      <span className="inline-flex justify-center">
                        <CoverageMark covered={entry.in_resume} />
                      </span>
                    </td>
                    <td className="px-2 py-1 text-center">
                      <span className="inline-flex justify-center">
                        <CoverageMark covered={entry.in_claims} />
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}
