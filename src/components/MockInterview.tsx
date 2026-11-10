import { useEffect, useState } from 'react';
import Markdown from './Markdown';

/**
 * The mock interview panel.
 *
 * `/mock` used to print one question into the command history and stop, so the
 * candidate had nothing to answer. This panel is the session: it states the
 * contract, shows progress, records each answer, and ends with a debrief.
 *
 * Field names mirror the Rust structs (snake_case) because they are serde
 * output, not local state.
 */

export interface MockQuestion {
  index: number;
  group: string;
  text: string;
  evidence: string;
  claim_ids: string[];
}

export interface MockPlan {
  role: string;
  mode: string;
  /** One of the ids in MOCK_PERSONAS. */
  persona: string;
  questions: MockQuestion[];
  grounded_in: number;
  posting_available: boolean;
  drill: boolean;
  contract: string;
}

export interface StarCoverage {
  situation: boolean;
  task: boolean;
  action: boolean;
  result: boolean;
}

export interface AnswerMetrics {
  words: number;
  wpm: number | null;
  fillers: string[];
  filler_count: number;
  hedges: string[];
  star: StarCoverage;
  has_metric: boolean;
  first_person_i: number;
  first_person_we: number;
}

export interface MockDimension {
  name: string;
  score: number;
  note: string;
}

export interface MockFeedback {
  overall: number;
  dimensions: MockDimension[];
  strengths: string[];
  fixes: string[];
  priority: string;
  stronger_answer: string | null;
  metrics: AnswerMetrics;
}

export interface MockEvaluation {
  ack: string;
  follow_up: string | null;
  feedback: MockFeedback | null;
  model_note: string | null;
}

export interface MockTurn {
  question: string;
  group: string;
  answer: string;
  followUpQuestion: string | null;
  followUpAnswer: string | null;
  feedback: MockFeedback | null;
  skipped: boolean;
}

export interface MockActiveAnswer {
  answer: string;
  followUp: string | null;
  ack: string | null;
}

export interface MockDrillResult {
  question: string;
  /** The best score this question had before the round. */
  previous: number;
  score: number;
  improved: boolean;
}

export interface MockDrillState {
  /** Best score per question, carried across rounds so a question cannot pass
   *  by scoring lower than it already has. */
  baseline: Record<string, number>;
  round: number;
  results: MockDrillResult[];
  /** The role the whole drill chain practises for. */
  role: string;
}

export interface MockSession {
  plan: MockPlan;
  transcript: MockTurn[];
  active: MockActiveAnswer | null;
  startedAt: number;
  questionStartedAt: number;
  status: 'asking' | 'complete';
  debrief: string | null;
  drill: MockDrillState | null;
}

const GROUP_LABELS: Record<string, string> = {
  technical: 'Technical',
  behavioral: 'Behavioral',
  'culture-fit': 'Culture fit',
};

export function groupLabel(group: string): string {
  return GROUP_LABELS[group] || group;
}

export interface MockPersonaOption {
  id: string;
  label: string;
  blurb: string;
}

/**
 * The interviewer voices a session can run in. The voice changes the tone of the
 * reactions and the coaching prose; it never changes a measured score, so a
 * harsh persona cannot manufacture a worse result.
 */
export const MOCK_PERSONAS: MockPersonaOption[] = [
  { id: 'supportive', label: 'Supportive', blurb: 'Warm; leads with what worked' },
  { id: 'professional', label: 'Professional', blurb: 'Neutral and specific' },
  { id: 'skeptical', label: 'Skeptical', blurb: 'Wants evidence for every claim' },
  { id: 'impatient', label: 'Impatient', blurb: 'Terse; pressed for time' },
];

export function personaLabel(id: string): string {
  return MOCK_PERSONAS.find((persona) => persona.id === id)?.label ?? 'Professional';
}

interface MockSetupPanelProps {
  role: string;
  mode: string;
  persona: string;
  busy: boolean;
  onPersona: (id: string) => void;
  onStart: () => void;
  onCancel: () => void;
}

/** The preflight card: who is interviewing you, before the first question. */
export function MockSetupPanel({
  role,
  mode,
  persona,
  busy,
  onPersona,
  onStart,
  onCancel,
}: MockSetupPanelProps) {
  return (
    <section className="border-b border-aether-border bg-aether-surface px-4 py-3">
      <p className="text-meta uppercase tracking-wide text-aether-text-muted">
        Mock interview · {mode === 'simulation' ? 'Simulation' : 'Coaching'}
      </p>
      <p className="mt-1 text-row font-semibold text-aether-text">{role}</p>

      <fieldset className="mt-3">
        <legend className="text-meta text-aether-text-muted">Who is interviewing you?</legend>
        <div className="mt-2 grid gap-2 sm:grid-cols-2">
          {MOCK_PERSONAS.map((option) => {
            const active = option.id === persona;
            return (
              <button
                key={option.id}
                type="button"
                aria-pressed={active}
                onClick={() => onPersona(option.id)}
                className={`rounded-lg border px-3 py-2 text-left transition ${
                  active
                    ? 'border-aether-primary bg-aether-primary/10'
                    : 'border-aether-border hover:bg-aether-bg'
                }`}
              >
                <span className="block text-body font-medium text-aether-text">
                  {option.label}
                </span>
                <span className="block text-meta text-aether-text-muted">{option.blurb}</span>
              </button>
            );
          })}
        </div>
      </fieldset>

      <div className="mt-3 flex flex-wrap items-center gap-2">
        <button
          onClick={onStart}
          disabled={busy}
          className="rounded-lg bg-aether-primary px-3 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
        >
          Start interview
        </button>
        <button
          onClick={onCancel}
          disabled={busy}
          className="rounded-lg border border-aether-border px-3 py-2 text-body text-aether-text transition hover:bg-aether-bg disabled:opacity-50"
        >
          Cancel
        </button>
        <span className="text-meta text-aether-text-muted">
          The tone changes; the scoring does not.
        </span>
      </div>
    </section>
  );
}

/**
 * The answers worth drilling: the lowest scores, at most three, and only ones
 * with room to improve. A run that scored well everywhere still drills its
 * single weakest answer rather than offering nothing.
 */
export function weakestTurns(transcript: MockTurn[]): MockTurn[] {
  const scored = transcript.filter(
    (turn): turn is MockTurn & { feedback: MockFeedback } =>
      !turn.skipped && turn.feedback !== null,
  );
  const sorted = [...scored].sort((a, b) => a.feedback.overall - b.feedback.overall);
  const under = sorted.filter((turn) => turn.feedback.overall < 85).slice(0, 3);
  return under.length > 0 ? under : sorted.slice(0, 1);
}

function overallTone(score: number): string {
  if (score >= 80) return 'text-aether-success';
  if (score >= 60) return 'text-aether-primary-ink';
  if (score >= 40) return 'text-aether-warning';
  return 'text-aether-error';
}

function starLine(metrics: AnswerMetrics): string {
  const parts = [
    metrics.star.situation && 'situation',
    metrics.star.task && 'task',
    metrics.star.action && 'action',
    metrics.star.result && 'result',
  ].filter(Boolean) as string[];
  return parts.length > 0 ? parts.join(', ') : 'none of it';
}

function MetricsLine({ metrics }: { metrics: AnswerMetrics }) {
  const pieces = [
    `${metrics.words} words`,
    metrics.wpm ? `~${metrics.wpm} wpm` : null,
    metrics.filler_count > 0 ? `${metrics.filler_count} filler` : null,
    metrics.hedges.length > 0 ? `hedging: ${metrics.hedges.join(', ')}` : null,
    `STAR: ${starLine(metrics)}`,
    metrics.has_metric ? 'a number' : 'no number',
  ].filter(Boolean);

  return <p className="mt-2 text-meta text-aether-text-muted">{pieces.join(' · ')}</p>;
}

export function FeedbackCard({ feedback }: { feedback: MockFeedback }) {
  return (
    <div className="mt-2 rounded-lg border border-aether-border bg-aether-surface p-3">
      <div className="flex items-center justify-between gap-2">
        <p className="text-meta uppercase tracking-wide text-aether-text-muted">Coach</p>
        <p className="font-mono text-body text-aether-text">
          <span className={overallTone(feedback.overall)}>{feedback.overall}</span>/100
        </p>
      </div>

      <div className="mt-2 flex flex-wrap gap-1">
        {feedback.dimensions.map((dimension) => (
          <span
            key={dimension.name}
            title={dimension.note}
            className="rounded bg-aether-bg px-1.5 py-0.5 text-meta text-aether-text-muted"
          >
            {dimension.name} {dimension.score}/5
          </span>
        ))}
      </div>

      {feedback.priority && (
        <p className="mt-2 text-body text-aether-text">
          <span className="text-aether-text-muted">Fix first: </span>
          {feedback.priority}
        </p>
      )}

      {feedback.strengths.length > 0 && (
        <ul className="mt-2 list-disc space-y-1 pl-4 text-meta text-aether-text-muted">
          {feedback.strengths.map((strength, index) => (
            <li key={`strength-${index}`}>{strength}</li>
          ))}
        </ul>
      )}

      {feedback.fixes.length > 0 && (
        <ul className="mt-2 list-disc space-y-1 pl-4 text-meta text-aether-warning">
          {feedback.fixes.map((fix, index) => (
            <li key={`fix-${index}`}>{fix}</li>
          ))}
        </ul>
      )}

      {feedback.stronger_answer && (
        <p className="mt-2 text-meta text-aether-text-muted">
          <span className="text-aether-text">Stronger: </span>
          {feedback.stronger_answer}
        </p>
      )}

      <MetricsLine metrics={feedback.metrics} />
    </div>
  );
}

interface MockSessionPanelProps {
  session: MockSession;
  busy: boolean;
  /** The candidate's answer for the question on screen. */
  onAnswer: (text: string) => void;
  onSkip: () => void;
  onEnd: () => void;
  onRetry: () => void;
  onRestart: () => void;
  /** Drill the weakest answers, or repeat the drill's unfinished ones. */
  onDrill: () => void;
  onDone: () => void;
}

export default function MockSessionPanel({
  session,
  busy,
  onAnswer,
  onSkip,
  onEnd,
  onRetry,
  onRestart,
  onDrill,
  onDone,
}: MockSessionPanelProps) {
  // The composer lives in the panel because the palette's input reads as a
  // command box; answering from here needs no keyboard knowledge. The stage key
  // clears the draft when the question or the follow-up changes.
  const [draft, setDraft] = useState('');
  const stage = `${session.transcript.length}:${session.active?.followUp ? 'follow-up' : 'question'}`;
  useEffect(() => {
    setDraft('');
  }, [stage]);

  const send = () => {
    const text = draft.trim();
    if (!text || busy) return;
    onAnswer(text);
  };

  const total = session.plan.questions.length;
  const answered = session.transcript.filter((turn) => !turn.skipped).length;
  const position = Math.min(session.transcript.length + 1, total);
  const percent = total === 0 ? 0 : Math.round((session.transcript.length / total) * 100);
  const current =
    session.status === 'asking' && session.transcript.length < total
      ? session.plan.questions[session.transcript.length]
      : null;
  const simulation = session.plan.mode === 'simulation';
  const weakest = session.drill ? [] : weakestTurns(session.transcript);
  const pending = session.drill
    ? session.drill.results.filter((result) => !result.improved)
    : [];

  return (
    <section className="mt-4 rounded-xl border border-aether-border bg-aether-surface">
      <header className="border-b border-aether-border p-3">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div className="min-w-0">
            <p className="flex flex-wrap items-center gap-2 text-meta uppercase tracking-wide text-aether-text-muted">
              Mock interview
              <span
                className={`rounded px-1.5 py-0.5 text-meta normal-case ${
                  session.drill
                    ? 'bg-aether-info/10 text-aether-info'
                    : simulation
                      ? 'bg-aether-warning/10 text-aether-warning'
                      : 'bg-aether-primary/10 text-aether-primary-ink'
                }`}
              >
                {session.drill
                  ? `Drill · round ${session.drill.round}`
                  : simulation
                    ? 'Simulation'
                    : 'Coaching'}
              </span>
              <span className="rounded bg-aether-bg px-1.5 py-0.5 text-meta normal-case text-aether-text-muted">
                {personaLabel(session.plan.persona)}
              </span>
            </p>
            <p className="truncate text-row font-semibold text-aether-text">{session.plan.role}</p>
          </div>

          <div className="flex items-center gap-2">
            {session.status === 'asking' ? (
              <>
                <span className="font-mono text-meta text-aether-text-muted">
                  Question {position} of {total}
                </span>
                <button
                  onClick={onEnd}
                  disabled={busy}
                  className="rounded border border-aether-border px-2 py-1 text-meta text-aether-text transition hover:bg-aether-bg disabled:opacity-50"
                >
                  End &amp; debrief
                </button>
              </>
            ) : (
              <button
                onClick={onDone}
                className="rounded border border-aether-border px-2 py-1 text-meta text-aether-text transition hover:bg-aether-bg"
              >
                Done
              </button>
            )}
          </div>
        </div>

        <div className="mt-2 h-1 w-full overflow-hidden rounded bg-aether-bg">
          <div
            className="h-full rounded bg-aether-primary transition-all"
            style={{ width: `${percent}%` }}
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={total}
            aria-valuenow={session.transcript.length}
            aria-label="Interview progress"
          />
        </div>
        <p className="mt-1 text-meta text-aether-text-muted">
          {session.plan.contract} · {answered} answered
        </p>
      </header>

      <div className="space-y-4 p-3">
        {session.transcript.map((turn, index) => (
          <div key={`turn-${index}`} className="space-y-2">
            <p className="text-meta uppercase tracking-wide text-aether-text-muted">
              Question {index + 1} · {groupLabel(turn.group)}
            </p>
            <p className="text-row text-aether-text">{turn.question}</p>

            <div className="rounded-lg border-l-2 border-aether-border bg-aether-bg/60 p-2">
              <p className="text-meta text-aether-text-muted">You</p>
              <p className="whitespace-pre-wrap text-body text-aether-text">
                {turn.skipped ? 'Skipped.' : turn.answer}
              </p>
              {turn.followUpQuestion && (
                <>
                  <p className="mt-2 text-meta text-aether-text-muted">Follow-up</p>
                  <p className="text-body text-aether-text">{turn.followUpQuestion}</p>
                  <p className="mt-1 whitespace-pre-wrap text-body text-aether-text">
                    {turn.followUpAnswer || 'Not answered.'}
                  </p>
                </>
              )}
            </div>

            {turn.feedback && <FeedbackCard feedback={turn.feedback} />}
            {turn.skipped && !turn.feedback && (
              <p className="text-meta text-aether-text-muted">No feedback — the question was skipped.</p>
            )}
          </div>
        ))}

        {session.active && (
          <div className="space-y-2">
            <div className="rounded-lg border-l-2 border-aether-border bg-aether-bg/60 p-2">
              <p className="text-meta text-aether-text-muted">You</p>
              <p className="whitespace-pre-wrap text-body text-aether-text">{session.active.answer}</p>
            </div>
            {session.active.ack && (
              <p className="text-body text-aether-text-muted">{session.active.ack}</p>
            )}
            {session.active.followUp && (
              <div className="rounded-lg border border-aether-info/40 bg-aether-info/5 p-2">
                <p className="text-meta uppercase tracking-wide text-aether-text-muted">Follow-up</p>
                <p className="text-row text-aether-text">{session.active.followUp}</p>
              </div>
            )}
          </div>
        )}

        {current && !session.active && (
          <div className="rounded-lg border border-aether-border bg-aether-bg/60 p-3">
            <p className="text-meta uppercase tracking-wide text-aether-text-muted">
              Question {current.index} · {groupLabel(current.group)}
            </p>
            <p className="mt-1 text-row text-aether-text">{current.text}</p>
            {!simulation && (
              <p className="mt-2 text-meta text-aether-text-muted">
                A strong answer includes: {current.evidence}
              </p>
            )}
          </div>
        )}

        {session.status === 'asking' && (
          <div className="rounded-lg border border-aether-border bg-aether-bg/60 p-2">
            <label
              htmlFor="mock-answer"
              className="text-meta uppercase tracking-wide text-aether-text-muted"
            >
              {session.active?.followUp
                ? 'Your answer to the follow-up'
                : `Your answer${current ? ` to question ${current.index}` : ''}`}
            </label>
            <textarea
              id="mock-answer"
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && !event.shiftKey) {
                  event.preventDefault();
                  send();
                }
              }}
              rows={3}
              disabled={busy}
              placeholder="Say it in your own words — a few sentences is enough."
              className="mt-1 w-full resize-y rounded border border-aether-border bg-aether-surface p-2 text-body text-aether-text outline-none placeholder:text-aether-text-muted/70 focus:border-aether-primary disabled:opacity-60"
            />
            <div className="mt-2 flex flex-wrap items-center gap-2">
              <button
                onClick={send}
                disabled={busy || draft.trim().length === 0}
                className="rounded-lg bg-aether-primary px-3 py-1.5 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
              >
                Send answer
              </button>
              <button
                onClick={onSkip}
                disabled={busy}
                className="rounded-lg border border-aether-border px-3 py-1.5 text-body text-aether-text transition hover:bg-aether-bg disabled:opacity-50"
              >
                Skip this question
              </button>
              <span className="text-meta text-aether-text-muted">
                Enter sends · Shift+Enter starts a new line
              </span>
            </div>
          </div>
        )}

        {busy && (
          <p className="flex items-center gap-2 text-meta text-aether-text-muted">
            <span className="h-2 w-2 animate-pulse rounded-full bg-aether-primary" />
            The interviewer is thinking…
          </p>
        )}

        {session.status === 'complete' && session.drill && (
          <div className="space-y-3 border-t border-aether-border pt-3">
            <p className="text-meta uppercase tracking-wide text-aether-text-muted">
              Drill results · round {session.drill.round}
            </p>
            <ul className="space-y-2">
              {session.drill.results.map((result, index) => (
                <li
                  key={`drill-${index}`}
                  className="rounded-lg border border-aether-border bg-aether-bg/60 p-2"
                >
                  <div className="flex items-start justify-between gap-2">
                    <p className="min-w-0 text-body text-aether-text">{result.question}</p>
                    <span
                      className={`shrink-0 font-mono text-meta ${
                        result.improved ? 'text-aether-success' : 'text-aether-warning'
                      }`}
                    >
                      {result.previous} → {result.score}
                    </span>
                  </div>
                  <p className="mt-1 text-meta text-aether-text-muted">
                    {result.improved
                      ? 'Improved — this one stays answered.'
                      : 'Still below your best. Say it again with that fix.'}
                  </p>
                </li>
              ))}
            </ul>
            <p className="text-body text-aether-text">
              {pending.length === 0
                ? `${session.drill.results.length} answer(s) improved. Nothing left to drill.`
                : `${pending.length} answer(s) still need work. The next round asks only those, against the same best score.`}
            </p>
            <div className="flex flex-wrap gap-2">
              {pending.length > 0 && (
                <button
                  onClick={onDrill}
                  disabled={busy}
                  className="rounded-lg bg-aether-primary px-3 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
                >
                  Drill again ({pending.length} left)
                </button>
              )}
              <button
                onClick={onRestart}
                disabled={busy}
                className="rounded-lg border border-aether-border px-3 py-2 text-body text-aether-text transition hover:bg-aether-bg disabled:opacity-50"
              >
                Full interview
              </button>
              <button
                onClick={onDone}
                className="rounded-lg border border-aether-border px-3 py-2 text-body text-aether-text transition hover:bg-aether-bg"
              >
                Close the session
              </button>
            </div>
          </div>
        )}

        {session.status === 'complete' && !session.drill && (
          <div className="space-y-3 border-t border-aether-border pt-3">
            {session.debrief ? (
              <Markdown content={session.debrief} />
            ) : (
              <p className="text-meta text-aether-text-muted">
                The debrief could not be generated. The answers above are still on screen.
              </p>
            )}
            {simulation && (
              <p className="text-meta text-aether-text-muted">
                Drills compare per-answer scores, so they start from a coaching run. `/mock{' '}
                {session.plan.role}` runs the same questions with feedback.
              </p>
            )}
            <div className="flex flex-wrap gap-2">
              {weakest.length > 0 && (
                <button
                  onClick={onDrill}
                  disabled={busy}
                  className="rounded-lg bg-aether-primary px-3 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
                >
                  Drill weak answers ({weakest.length})
                </button>
              )}
              <button
                onClick={onRestart}
                disabled={busy}
                className={`rounded-lg px-3 py-2 text-body transition disabled:opacity-50 ${
                  weakest.length > 0
                    ? 'border border-aether-border text-aether-text hover:bg-aether-bg'
                    : 'bg-aether-primary font-medium text-aether-primary-fg hover:bg-aether-primary-hover'
                }`}
              >
                Practice again
              </button>
              <button
                onClick={onDone}
                className="rounded-lg border border-aether-border px-3 py-2 text-body text-aether-text transition hover:bg-aether-bg"
              >
                Close the session
              </button>
            </div>
          </div>
        )}

        {session.status === 'asking' && session.transcript.length > 0 && !busy && (
          <button
            onClick={onRetry}
            className="text-meta text-aether-text-muted underline underline-offset-2 transition hover:text-aether-text"
          >
            Redo the last answer
          </button>
        )}
      </div>
    </section>
  );
}
