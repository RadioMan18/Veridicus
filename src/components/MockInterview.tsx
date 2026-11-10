import { useEffect, useRef, useState } from 'react';
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
  /** The question group the run leans on, or null for all groups. */
  focus: string | null;
  /** The application this run practises for, or null when none matched. */
  application_id: string | null;
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
  /** Seconds the session clock measured, or null when it sent none. */
  elapsed_seconds: number | null;
  /** short | on-target | over | long against the soft target. */
  pacing: string | null;
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
  /** The score the replaced attempt got; null when this is not a redo. */
  previousScore: number | null;
  /** Fixes the replaced attempt listed that this one no longer does. */
  addressed: string[];
}

/** The attempt a redo is replacing, held until the question is answered again. */
export interface PreviousAttempt {
  score: number;
  fixes: string[];
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

/** One finished run, as the history strip reads it. */
export interface MockSessionRecord {
  /** The application the run was practising for, or null. */
  application_id: string | null;
  /** `YYYY-MM-DD HH:MM`, local time. */
  date: string;
  role: string;
  mode: string;
  persona: string;
  answered: number;
  asked: number;
  average: number;
  /** The lowest-scoring dimension of the run, or an empty string. */
  weakest: string;
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
  /** Set by a redo, cleared once the question is answered or skipped again. */
  previousAttempt: PreviousAttempt | null;
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

/** The lengths the card offers; the backend clamps to the same range. */
export const MOCK_LENGTHS = [3, 6, 12];

/** `all` keeps the usual mix; a group is asked before the others. */
export const MOCK_FOCUS_AREAS = [
  { id: 'all', label: 'All groups' },
  { id: 'technical', label: 'Technical' },
  { id: 'behavioral', label: 'Behavioral' },
  { id: 'culture-fit', label: 'Culture fit' },
];

const MOCK_MODES = [
  { id: 'coaching', label: 'Coaching', blurb: 'feedback after every answer' },
  { id: 'simulation', label: 'Simulation', blurb: 'a verdict in the debrief' },
];

/** One application a run can file under. */
export interface MockApplicationOption {
  id: string;
  role: string;
  employer: string;
}

interface MockSetupPanelProps {
  role: string;
  mode: string;
  persona: string;
  questionCount: number;
  focus: string;
  /** The application the run files under, or null for none. */
  applicationId: string | null;
  /** The applications the picker offers, newest first. */
  applications: MockApplicationOption[];
  history: MockSessionRecord[];
  busy: boolean;
  onRole: (role: string) => void;
  onMode: (mode: string) => void;
  onCount: (count: number) => void;
  onFocus: (focus: string) => void;
  onApplication: (id: string | null) => void;
  onPersona: (id: string) => void;
  onStart: () => void;
  onCancel: () => void;
}

function setupChoiceClass(active: boolean): string {
  return `rounded-lg border px-3 py-1.5 text-left transition ${
    active
      ? 'border-aether-primary bg-aether-primary/10'
      : 'border-aether-border hover:bg-aether-bg'
  }`;
}

/**
 * The preflight card: what the run asks, how long it runs, and who asks it. The
 * role starts on the latest application, and every choice the flags used to
 * carry is visible here.
 */
/** The last few runs, so the candidate can see what they are improving on. */
export function MockHistoryStrip({
  sessions,
  title = 'Recent sessions',
}: {
  sessions: MockSessionRecord[];
  title?: string;
}) {
  if (sessions.length === 0) return null;
  return (
    <div className="mt-3">
      <p className="text-meta uppercase tracking-wide text-aether-text-muted">{title}</p>
      <ul className="mt-1 space-y-1">
        {sessions.map((session, index) => (
          <li
            key={`${session.date}-${index}`}
            className="flex flex-wrap items-baseline gap-x-2 text-meta text-aether-text-muted"
          >
            <span className="font-mono">{session.date}</span>
            <span className="min-w-0 flex-1 truncate text-aether-text">{session.role}</span>
            <span className="font-mono text-aether-text">{session.average}/100</span>
            {session.weakest && (
              <span>
                weakest: {session.weakest} · {session.answered} of {session.asked} answered
              </span>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

export function MockSetupPanel({
  role,
  mode,
  persona,
  questionCount,
  focus,
  applicationId,
  applications,
  history,
  busy,
  onRole,
  onMode,
  onCount,
  onFocus,
  onApplication,
  onPersona,
  onStart,
  onCancel,
}: MockSetupPanelProps) {
  return (
    <section className="max-h-[65vh] overflow-y-auto border-b border-aether-border bg-aether-surface px-4 py-3">
      <p className="text-meta uppercase tracking-wide text-aether-text-muted">
        Mock interview · {mode === 'simulation' ? 'Simulation' : 'Coaching'}
      </p>

      <div className="mt-2">
        <label htmlFor="mock-setup-role" className="text-meta text-aether-text-muted">
          Role
        </label>
        <input
          id="mock-setup-role"
          value={role}
          onChange={(event) => onRole(event.target.value)}
          placeholder="Director of Training"
          className="mt-1 w-full rounded border border-aether-border bg-aether-bg px-2 py-1.5 text-body text-aether-text outline-none placeholder:text-aether-text-muted/70 focus:border-aether-primary"
        />
      </div>

      {applications.length > 0 && (
        <div className="mt-3">
          <label
            htmlFor="mock-setup-application"
            className="text-meta text-aether-text-muted"
          >
            Application
          </label>
          <select
            id="mock-setup-application"
            value={applicationId ?? ''}
            onChange={(event) => onApplication(event.target.value || null)}
            className="mt-1 w-full rounded border border-aether-border bg-aether-bg px-2 py-1.5 text-body text-aether-text outline-none focus:border-aether-primary"
          >
            <option value="">None — practice without an application</option>
            {applications.map((application) => (
              <option key={application.id} value={application.id}>
                {application.employer} — {application.role}
              </option>
            ))}
          </select>
          <p className="mt-1 text-meta text-aether-text-muted">
            The debrief is saved in this application's folder, and the run shows up in its prep
            view.
          </p>
        </div>
      )}

      <fieldset className="mt-3">
        <legend className="text-meta text-aether-text-muted">Feedback</legend>
        <div className="mt-2 flex flex-wrap gap-2">
          {MOCK_MODES.map((option) => (
            <button
              key={option.id}
              type="button"
              aria-pressed={option.id === mode}
              onClick={() => onMode(option.id)}
              className={setupChoiceClass(option.id === mode)}
            >
              <span className="block text-body text-aether-text">{option.label}</span>
              <span className="block text-meta text-aether-text-muted">{option.blurb}</span>
            </button>
          ))}
        </div>
      </fieldset>

      <fieldset className="mt-3">
        <legend className="text-meta text-aether-text-muted">Questions</legend>
        <div className="mt-2 flex flex-wrap items-center gap-2">
          {MOCK_LENGTHS.map((count) => (
            <button
              key={count}
              type="button"
              aria-pressed={count === questionCount}
              onClick={() => onCount(count)}
              className={`${setupChoiceClass(count === questionCount)} font-mono text-body ${
                count === questionCount ? 'text-aether-text' : 'text-aether-text-muted'
              }`}
            >
              {count}
            </button>
          ))}
        </div>
      </fieldset>

      <fieldset className="mt-3">
        <legend className="text-meta text-aether-text-muted">Focus</legend>
        <div className="mt-2 flex flex-wrap gap-2">
          {MOCK_FOCUS_AREAS.map((option) => (
            <button
              key={option.id}
              type="button"
              aria-pressed={option.id === focus}
              onClick={() => onFocus(option.id)}
              className={`${setupChoiceClass(option.id === focus)} text-body text-aether-text`}
            >
              {option.label}
            </button>
          ))}
        </div>
        <p className="mt-1 text-meta text-aether-text-muted">
          The focus group is asked first; the other groups fill the run.
        </p>
      </fieldset>

      <MockHistoryStrip sessions={history} />

      <fieldset className="mt-3">
        <legend className="text-meta text-aether-text-muted">Who is interviewing you?</legend>
        <div className="mt-2 grid gap-2 sm:grid-cols-2">
          {MOCK_PERSONAS.map((option) => (
            <button
              key={option.id}
              type="button"
              aria-pressed={option.id === persona}
              onClick={() => onPersona(option.id)}
              className={setupChoiceClass(option.id === persona)}
            >
              <span className="block text-body font-medium text-aether-text">
                {option.label}
              </span>
              <span className="block text-meta text-aether-text-muted">{option.blurb}</span>
            </button>
          ))}
        </div>
      </fieldset>

      <div className="mt-3 flex flex-wrap items-center gap-2">
        <button
          onClick={onStart}
          disabled={busy || role.trim().length === 0}
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

/** m:ss, as both the live clock and the pacing line show it. */
function clockLabel(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`;
}

const STAR_PARTS: { key: keyof StarCoverage; label: string; missing: string }[] = [
  {
    key: 'situation',
    label: 'Situation',
    missing: 'no situation — open with when, where and what was at stake',
  },
  { key: 'task', label: 'Task', missing: 'no task — say what you were asked to do' },
  { key: 'action', label: 'Action', missing: 'no action — say what you actually did' },
  {
    key: 'result',
    label: 'Result',
    missing: 'no result — say what changed and how you measured it',
  },
];

/** STAR as four ticks, so a missing part is visible without reading the prose. */
function StarChecklist({ star }: { star: StarCoverage }) {
  const missing = STAR_PARTS.filter((part) => !star[part.key]);
  return (
    <div className="mt-2">
      <ul className="flex flex-wrap gap-x-3 gap-y-1 text-meta">
        {STAR_PARTS.map((part) => {
          const present = star[part.key];
          return (
            <li
              key={part.key}
              className={present ? 'text-aether-success' : 'text-aether-text-muted'}
            >
              <span aria-hidden="true">{present ? '✓' : '○'}</span> {part.label}
              <span className="sr-only">{present ? ' is present' : ' is missing'}</span>
            </li>
          );
        })}
      </ul>
      {missing.length > 0 && (
        <ul className="mt-1 list-disc space-y-1 pl-4 text-meta text-aether-warning">
          {missing.map((part) => (
            <li key={part.key}>{part.missing}</li>
          ))}
        </ul>
      )}
    </div>
  );
}

const PACING_COPY: Record<string, string> = {
  short: 'short — a fuller answer gives the interviewer more to work with',
  'on-target': 'on target — 90–120s is typical',
  over: 'over target — 90–120s is typical, 3 minutes is long',
  long: 'long — 3 minutes is long; aim for 90–120s',
};

/** How long the answer took, read against the soft target. Never a grade. */
function PacingLine({ metrics }: { metrics: AnswerMetrics }) {
  if (metrics.elapsed_seconds === null) return null;
  const band = metrics.pacing ?? '';
  const late = band === 'over' || band === 'long';
  return (
    <p className={`mt-2 text-meta ${late ? 'text-aether-warning' : 'text-aether-text-muted'}`}>
      <span className="font-mono">{clockLabel(metrics.elapsed_seconds)}</span>
      {' · '}
      {PACING_COPY[band] ?? 'pacing was not measured'}
    </p>
  );
}

function MetricsLine({ metrics }: { metrics: AnswerMetrics }) {
  const pieces = [
    `${metrics.words} words`,
    metrics.wpm ? `~${metrics.wpm} wpm` : null,
    metrics.filler_count > 0 ? `${metrics.filler_count} filler` : null,
    metrics.hedges.length > 0 ? `hedging: ${metrics.hedges.join(', ')}` : null,
    metrics.has_metric ? 'a number' : 'no number',
  ].filter(Boolean);

  return <p className="mt-2 text-meta text-aether-text-muted">{pieces.join(' · ')}</p>;
}

export function FeedbackCard({
  feedback,
  previousScore = null,
  addressed = [],
  showScore = true,
}: {
  feedback: MockFeedback;
  /** The score the previous attempt got, when this answer is a redo. */
  previousScore?: number | null;
  /** Fixes the previous attempt listed that this one no longer does. */
  addressed?: string[];
  /** Coaching leads with the fix; simulation keeps the verdict on top. */
  showScore?: boolean;
}) {
  const fixLine = feedback.priority ? (
    <p className="mt-2 text-body text-aether-text">
      <span className="text-aether-text-muted">Fix first: </span>
      {feedback.priority}
    </p>
  ) : null;

  const dimensions = (
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
  );

  return (
    <div className="mt-2 rounded-lg border border-aether-border bg-aether-surface p-3">
      <div className="flex items-center justify-between gap-2">
        <p className="text-meta uppercase tracking-wide text-aether-text-muted">Coach</p>
        {showScore && (
          <p className="font-mono text-body text-aether-text">
            {previousScore !== null && (
              <span className="text-aether-text-muted">{previousScore} → </span>
            )}
            <span className={overallTone(feedback.overall)}>{feedback.overall}</span>/100
          </p>
        )}
      </div>

      {showScore ? (
        <>
          {dimensions}
          <StarChecklist star={feedback.metrics.star} />
          {fixLine}
        </>
      ) : (
        <>
          {fixLine}
          <StarChecklist star={feedback.metrics.star} />
          {dimensions}
        </>
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

      {addressed.length > 0 && (
        <>
          <p className="mt-2 text-meta text-aether-success">Addressed since the last attempt</p>
          <ul className="mt-1 list-disc space-y-1 pl-4 text-meta text-aether-success">
            {addressed.map((fix, index) => (
              <li key={`addressed-${index}`}>{fix}</li>
            ))}
          </ul>
        </>
      )}

      {feedback.stronger_answer && (
        <p className="mt-2 text-meta text-aether-text-muted">
          <span className="text-aether-text">Stronger: </span>
          {feedback.stronger_answer}
        </p>
      )}

      <PacingLine metrics={feedback.metrics} />
      <MetricsLine metrics={feedback.metrics} />
    </div>
  );
}

/**
 * One answered question. Its coach card folds into a chip so a long run stays
 * short: the newest card is open, and opening an older one closes the rest.
 */
function TranscriptTurn({
  turn,
  index,
  open,
  onToggle,
  showScore,
}: {
  turn: MockTurn;
  index: number;
  open: boolean;
  onToggle: () => void;
  /** Simulation and drills show the grade; coaching hides it. */
  showScore: boolean;
}) {
  // A redo keeps its comparison even in coaching: beating your own last score
  // is the point of retrying, not a grade.
  const showGrade = showScore || turn.previousScore !== null;
  return (
    <div className="space-y-2">
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

      {turn.feedback && (
        <>
          <button
            type="button"
            aria-expanded={open}
            onClick={onToggle}
            className="flex w-full items-center justify-between gap-2 rounded border border-aether-border px-2 py-1 text-meta text-aether-text-muted transition hover:bg-aether-bg"
          >
            <span className="font-mono">
              Question {index + 1}
              {showGrade && (
                <>
                  {' · '}
                  {turn.previousScore !== null && (
                    <span className="text-aether-text-muted">{turn.previousScore} → </span>
                  )}
                  <span className={overallTone(turn.feedback.overall)}>
                    {turn.feedback.overall}
                  </span>
                  /100
                </>
              )}
            </span>
            <span>{open ? 'Hide coach' : 'Show coach'}</span>
          </button>
          {open && (
            <FeedbackCard
              feedback={turn.feedback}
              previousScore={turn.previousScore}
              addressed={turn.addressed}
              showScore={showScore}
            />
          )}
        </>
      )}
      {!turn.feedback && turn.skipped && (
        <p className="text-meta text-aether-text-muted">No feedback — the question was skipped.</p>
      )}
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
  onCopyDebrief: () => void;
  onSaveDebrief: () => void;
  onPushDebrief: () => void;
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
  onCopyDebrief,
  onSaveDebrief,
  onPushDebrief,
}: MockSessionPanelProps) {
  // The composer lives in the panel because the palette's input reads as a
  // command box; answering from here needs no keyboard knowledge. The stage key
  // clears the draft when the question or the follow-up changes.
  const [draft, setDraft] = useState('');
  const stage = `${session.transcript.length}:${session.active?.followUp ? 'follow-up' : 'question'}`;
  useEffect(() => {
    setDraft('');
  }, [stage]);

  // The clock times the part being answered: it restarts on a follow-up, as
  // chosen, and pauses while the interviewer is thinking, because that wait is
  // not the candidate's. The score still measures the whole question.
  const [clockSeconds, setClockSeconds] = useState(0);
  useEffect(() => {
    setClockSeconds(0);
  }, [stage]);
  useEffect(() => {
    if (busy || session.status !== 'asking') return;
    const timer = window.setInterval(() => setClockSeconds((value) => value + 1), 1000);
    return () => window.clearInterval(timer);
  }, [busy, session.status]);

  // Coach cards are a stack: the newest is open, clicking a chip opens that one
  // and closes the rest, and a new answer resets the stack to the newest.
  const [openTurn, setOpenTurn] = useState<number | null>(null);
  useEffect(() => {
    setOpenTurn(null);
  }, [session.transcript.length]);
  const latestTurn = session.transcript.length - 1;
  const turnOpen = (index: number) =>
    openTurn === null ? index === latestTurn : openTurn === index;

  // The transcript scrolls on its own, so follow the newest answer.
  const transcriptRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (session.status !== 'asking' || !transcriptRef.current) return;
    transcriptRef.current.scrollTop = transcriptRef.current.scrollHeight;
  }, [session.transcript.length, session.active?.followUp, session.status]);

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
  // Simulation is a verdict and a drill is measured against itself, so both
  // show the number; plain coaching keeps the grade out of the conversation.
  const showGrade = simulation || Boolean(session.drill);
  const weakest = session.drill ? [] : weakestTurns(session.transcript);
  const pending = session.drill
    ? session.drill.results.filter((result) => !result.improved)
    : [];
  const asking = session.status === 'asking';
  const activeFollowUp = session.active?.followUp ?? null;

  return (
    <section
      className={`mt-4 flex flex-col rounded-xl border border-aether-border bg-aether-surface ${
        asking ? 'max-h-[60vh]' : ''
      }`}
    >
      <header className="shrink-0 border-b border-aether-border p-3">
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

      {/* The transcript scrolls inside the panel; the question and composer
          below it never move, so a long run cannot push them out of view. */}
      <div ref={transcriptRef} className="min-h-0 flex-1 space-y-4 overflow-y-auto p-3">
        {session.transcript.map((turn, index) => (
          <TranscriptTurn
            key={`turn-${index}`}
            turn={turn}
            index={index}
            open={turnOpen(index)}
            showScore={showGrade}
            onToggle={() => setOpenTurn(turnOpen(index) ? -1 : index)}
          />
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
          </div>
        )}
      </div>

      {asking && (
        <div className="shrink-0 space-y-2 border-t border-aether-border p-3">
          {activeFollowUp ? (
            <div className="rounded-lg border border-aether-info/40 bg-aether-info/5 p-2">
              <div className="flex items-baseline justify-between gap-2">
                <p className="text-meta uppercase tracking-wide text-aether-text-muted">
                  Follow-up
                </p>
                <span className="font-mono text-meta text-aether-text-muted">
                  {clockLabel(clockSeconds)}
                </span>
              </div>
              <p className="text-row text-aether-text">{activeFollowUp}</p>
              <p className="mt-1 text-meta text-aether-text-muted">
                The clock restarted for the follow-up; the score covers the whole question.
              </p>
            </div>
          ) : (
            current && (
              <div className="rounded-lg border border-aether-border bg-aether-bg/60 p-3">
                <div className="flex items-baseline justify-between gap-2">
                  <p className="text-meta uppercase tracking-wide text-aether-text-muted">
                    Question {current.index} · {groupLabel(current.group)}
                  </p>
                  <span
                    className={`font-mono text-meta ${
                      clockSeconds > 120 ? 'text-aether-warning' : 'text-aether-text-muted'
                    }`}
                  >
                    <span className="sr-only">Time on this answer: </span>
                    {clockLabel(clockSeconds)}
                  </span>
                </div>
                <p className="mt-1 text-row text-aether-text">{current.text}</p>
                {!simulation && (
                  <p className="mt-2 text-meta text-aether-text-muted">
                    A strong answer includes: {current.evidence}
                  </p>
                )}
                <p className="mt-2 text-meta text-aether-text-muted">
                  90–120s is typical; 3 minutes is long.
                </p>
              </div>
            )
          )}

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

          {busy && (
            <p className="flex items-center gap-2 text-meta text-aether-text-muted">
              <span className="h-2 w-2 animate-pulse rounded-full bg-aether-primary" />
              The interviewer is thinking…
            </p>
          )}

          {session.transcript.length > 0 && !busy && (
            <button
              onClick={onRetry}
              className="text-meta text-aether-text-muted underline underline-offset-2 transition hover:text-aether-text"
            >
              Redo the last answer
            </button>
          )}
        </div>
      )}

      <div className={asking ? '' : 'space-y-3 p-3'}>
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
              <button
                onClick={onCopyDebrief}
                disabled={!session.debrief}
                className="rounded-lg border border-aether-border px-3 py-2 text-body text-aether-text transition hover:bg-aether-bg disabled:opacity-50"
              >
                Copy debrief
              </button>
              <button
                onClick={onSaveDebrief}
                disabled={busy || !session.debrief}
                className="rounded-lg border border-aether-border px-3 py-2 text-body text-aether-text transition hover:bg-aether-bg disabled:opacity-50"
              >
                Save to Documents
              </button>
              <button
                onClick={onPushDebrief}
                disabled={!session.debrief}
                className="rounded-lg border border-aether-border px-3 py-2 text-body text-aether-text transition hover:bg-aether-bg disabled:opacity-50"
              >
                Add to history
              </button>
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
      </div>
    </section>
  );
}
