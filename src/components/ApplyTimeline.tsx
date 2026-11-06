import { CheckCircle2, Circle, Loader2, XCircle } from 'lucide-react';

export type StageStatus = 'pending' | 'active' | 'done' | 'failed' | 'awaiting';

export interface StageState {
  status: StageStatus;
  detail?: string;
}

export const APPLY_STAGES = [
  { id: 'scrape', label: 'Scrape job posting' },
  { id: 'gap', label: 'Gap analysis against claims ledger' },
  { id: 'resume', label: 'Tailor resume' },
  { id: 'letter', label: 'Write cover letter' },
  { id: 'fact-check', label: 'Fact-check both documents' },
  { id: 'render', label: 'Compile PDFs and run visual QA' },
  { id: 'save', label: 'Save, sync ledger and commit' },
] as const;

export function initialStageStates(): Record<string, StageState> {
  const states: Record<string, StageState> = {};
  for (const stage of APPLY_STAGES) {
    states[stage.id] = { status: 'pending' };
  }
  return states;
}

function stageIcon(status: StageStatus) {
  switch (status) {
    case 'done':
      return <CheckCircle2 size={16} className="text-aether-success" />;
    case 'active':
      return <Loader2 size={16} className="text-aether-primary animate-spin" />;
    case 'failed':
      return <XCircle size={16} className="text-aether-error" />;
    case 'awaiting':
      return <Circle size={16} className="text-aether-warning" />;
    default:
      return <Circle size={16} className="text-aether-text-muted opacity-50" />;
  }
}

export function formatElapsed(milliseconds: number) {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  return `${minutes}:${String(totalSeconds % 60).padStart(2, '0')}`;
}

interface ApplyTimelineProps {
  states: Record<string, StageState>;
  elapsedMs: number;
  model?: string;
  running: boolean;
  onCancel?: () => void;
}

export default function ApplyTimeline({
  states,
  elapsedMs,
  model,
  running,
  onCancel,
}: ApplyTimelineProps) {
  return (
    <div className="border border-aether-border rounded-lg bg-aether-bg p-3">
      <div className="mb-2 flex items-center justify-between text-xs text-aether-text-muted">
        <span className="truncate">{model ? `Local model: ${model}` : 'Local model'}</span>
        <span className="font-mono tabular-nums">{formatElapsed(elapsedMs)}</span>
      </div>

      <ol className="space-y-1.5">
        {APPLY_STAGES.map((stage) => {
          const state = states[stage.id] || { status: 'pending' as StageStatus };
          return (
            <li key={stage.id} className="flex items-start gap-2 text-sm leading-5">
              <span className="mt-0.5 shrink-0">{stageIcon(state.status)}</span>
              <span className="min-w-0 flex-1">
                <span
                  className={
                    state.status === 'pending'
                      ? 'text-aether-text-muted'
                      : state.status === 'failed'
                        ? 'text-aether-error'
                        : 'text-aether-text'
                  }
                >
                  {stage.label}
                </span>
                {state.detail && (
                  <span className="block truncate text-xs text-aether-text-muted" title={state.detail}>
                    {state.detail}
                  </span>
                )}
              </span>
            </li>
          );
        })}
      </ol>

      {running && onCancel && (
        <button
          onClick={onCancel}
          className="mt-3 rounded border border-aether-border px-2 py-1 text-xs text-aether-text-muted transition hover:border-aether-error hover:text-aether-error"
        >
          Cancel run
        </button>
      )}
    </div>
  );
}
