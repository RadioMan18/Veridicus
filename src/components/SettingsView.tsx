import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { readChimeMode, writeChimeMode, type ChimeMode } from '../lib/completionSound';
import {
  ensureNotificationPermission,
  isTauriRuntime,
  readNotifyEnabled,
  writeNotifyEnabled,
} from '../lib/desktopNotify';

interface SettingsViewProps {
  workspacePath: string;
  gitActive: boolean;
  setupComplete: boolean;
  onOpenModels: () => void;
  onOpenSetup: () => void;
}

interface HonestyStats {
  applications: number;
  generated: number;
  approved: number;
  overrides: number;
  blocked_findings: number;
  advisory_findings: number;
  by_code: Record<string, number>;
  blocks_by_code: Record<string, number>;
  false_block_reports: number;
  override_rate: number;
  last_review_at: string | null;
  next_review_due: string | null;
  review_cadence_days: number;
  review_overdue: boolean;
}

function formatTime(value: string | null) {
  if (!value) return 'Never';
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleString();
}

export default function SettingsView({
  workspacePath,
  gitActive,
  setupComplete,
  onOpenModels,
  onOpenSetup,
}: SettingsViewProps) {
  const [stats, setStats] = useState<HonestyStats | null>(null);
  const [statsError, setStatsError] = useState('');
  const [reviewer, setReviewer] = useState('local-user');
  const [reviewNote, setReviewNote] = useState('');
  const [recording, setRecording] = useState(false);
  const [reviewMessage, setReviewMessage] = useState('');
  const [chimeMode, setChimeMode] = useState<ChimeMode>(readChimeMode);
  const [notifyEnabled, setNotifyEnabled] = useState<boolean>(readNotifyEnabled);
  const [notifyHint, setNotifyHint] = useState('');

  useEffect(() => {
    let active = true;
    invoke<HonestyStats>('get_honesty_stats')
      .then((value) => {
        if (active) {
          setStats(value);
          setStatsError('');
        }
      })
      .catch((error: unknown) => {
        if (active) setStatsError(`Could not read the honesty record: ${String(error)}`);
      });
    return () => {
      active = false;
    };
  }, []);

  const recordReview = async () => {
    setRecording(true);
    setReviewMessage('');
    try {
      const updated = await invoke<HonestyStats>('record_honesty_review', {
        reviewer,
        note: reviewNote,
      });
      setStats(updated);
      setReviewNote('');
      setReviewMessage('Review recorded. The next due date has been reset.');
    } catch (error: unknown) {
      setReviewMessage(`Could not record the review: ${String(error)}`);
    } finally {
      setRecording(false);
    }
  };

  // Turning the toggle on is the consent step: ask once, here, never mid-run.
  const changeNotify = async (enabled: boolean) => {
    setNotifyEnabled(enabled);
    writeNotifyEnabled(enabled);
    setNotifyHint('');
    if (!enabled) return;
    if (!isTauriRuntime()) {
      setNotifyHint('Desktop notifications need the installed app; this browser preview cannot show them.');
      return;
    }
    const granted = await ensureNotificationPermission();
    if (granted) return;
    setNotifyEnabled(false);
    writeNotifyEnabled(false);
    setNotifyHint(
      'Windows would not grant notifications. Turn Veridicus on under Settings → System → Notifications.',
    );
  };

  const blocks = Object.entries(stats?.blocks_by_code || {});
  const overrideRate = Math.round((stats?.override_rate || 0) * 100);

  return (
    <div className="max-w-3xl space-y-4">
      <div>
        <h2 className="text-section text-aether-text">Settings</h2>
        <p className="text-body text-aether-text-muted">
          Local models, workspace and first-run setup.
        </p>
      </div>

      <section className="divide-y divide-aether-border rounded-xl border border-aether-border bg-aether-surface">
        <div className="flex flex-wrap items-center justify-between gap-3 p-4">
          <div>
            <p className="text-row text-aether-text">Local models</p>
            <p className="text-meta text-aether-text-muted">
              Choose the fast and deep Ollama models used for generation.
            </p>
          </div>
          <button
            onClick={onOpenModels}
            className="rounded border border-aether-border px-3 py-1.5 text-sm text-aether-text transition hover:bg-aether-bg"
          >
            Configure
          </button>
        </div>

        <div className="flex flex-wrap items-center justify-between gap-3 p-4">
          <div>
            <p className="text-row text-aether-text">Setup guide</p>
            <p className="text-meta text-aether-text-muted">
              {setupComplete
                ? 'Completed. Reopen to review your profile and claims.'
                : 'Not finished yet — import your resume and confirm claims.'}
            </p>
          </div>
          <button
            onClick={onOpenSetup}
            className="rounded border border-aether-border px-3 py-1.5 text-sm text-aether-text transition hover:bg-aether-bg"
          >
            Open wizard
          </button>
        </div>

        <div className="p-4">
          <p className="text-sm text-aether-text">Workspace</p>
          <p className="mt-1 break-all font-mono text-xs text-aether-text-muted">
            {workspacePath || 'Loading…'}
          </p>
          <p className="mt-1 text-xs text-aether-text-muted">
            {gitActive
              ? 'Local git history is active for your applications.'
              : 'No local git history yet.'}
          </p>
        </div>
      </section>

      <section className="rounded-xl border border-aether-border bg-aether-surface p-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-row text-aether-text">Completion sound</p>
            <p className="text-meta text-aether-text-muted">
              A short chime when a run takes longer than eight seconds. Failures use a lower tone;
              cancelled runs stay silent.
            </p>
          </div>
          <div className="flex overflow-hidden rounded border border-aether-border">
            {([
              ['off', 'Off'],
              ['background', 'Background'],
              ['always', 'Always'],
            ] as [ChimeMode, string][]).map(([mode, label]) => (
              <button
                key={mode}
                onClick={() => {
                  setChimeMode(mode);
                  writeChimeMode(mode);
                }}
                aria-pressed={chimeMode === mode}
                className={`px-3 py-1.5 text-sm transition ${
                  chimeMode === mode
                    ? 'bg-aether-primary text-aether-primary-fg'
                    : 'bg-aether-bg text-aether-text hover:border-aether-primary'
                }`}
              >
                {label}
              </button>
            ))}
          </div>
        </div>
        <div className="mt-4 border-t border-aether-border pt-4">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <p className="text-row text-aether-text">Desktop notifications</p>
              <p className="text-meta text-aether-text-muted">
                A Windows toast when a run finishes after eight seconds in the background. The copy
                names only the operation — never an employer or document text.
              </p>
            </div>
            <div className="flex overflow-hidden rounded border border-aether-border">
              {([
                [true, 'On'],
                [false, 'Off'],
              ] as [boolean, string][]).map(([enabled, label]) => (
                <button
                  key={label}
                  onClick={() => void changeNotify(enabled)}
                  aria-pressed={notifyEnabled === enabled}
                  className={`px-3 py-1.5 text-sm transition ${
                    notifyEnabled === enabled
                      ? 'bg-aether-primary text-aether-primary-fg'
                      : 'bg-aether-bg text-aether-text hover:border-aether-primary'
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
          {notifyHint && <p className="mt-2 text-meta text-aether-text-muted">{notifyHint}</p>}
        </div>
      </section>

      <section className="rounded-xl border border-aether-border bg-aether-surface p-4">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <h3 className="text-row font-semibold text-aether-text">Honesty record</h3>
            <p className="mt-1 text-meta text-aether-text-muted">
              Blocking findings, overrides, advisory findings and user-reported incidents. Review
              every {stats?.review_cadence_days ?? 90} days.
            </p>
          </div>
          {stats && (
            <span
              className={`rounded-full border px-2 py-0.5 text-meta ${
                stats.review_overdue || !stats.last_review_at
                  ? 'border-aether-warning text-aether-warning'
                  : 'border-aether-success text-aether-success'
              }`}
            >
              {!stats.last_review_at
                ? 'No review recorded'
                : stats.review_overdue
                  ? 'Review overdue'
                  : 'Review current'}
            </span>
          )}
        </div>

        {statsError && <p className="mt-3 text-meta text-aether-error">{statsError}</p>}

        {stats && (
          <>
            <div className="mt-3 grid grid-cols-2 gap-2 sm:grid-cols-4">
              {[
                ['Applications', stats.applications],
                ['Generations', stats.generated],
                ['Approved', stats.approved],
                ['Overrides', stats.overrides],
                ['Blocking findings', stats.blocked_findings],
                ['Advisory findings', stats.advisory_findings],
                ['False-block reports', stats.false_block_reports],
                ['Override rate', `${overrideRate}%`],
              ].map(([label, value]) => (
                <div key={String(label)} className="rounded-lg bg-aether-bg p-2.5">
                  <p className="text-meta text-aether-text-muted">{label}</p>
                  <p className="text-row font-semibold text-aether-text">{value}</p>
                </div>
              ))}
            </div>

            {blocks.length > 0 && (
              <p className="mt-3 text-meta text-aether-text-muted">
                Blocks by code:{' '}
                {blocks.map(([code, count]) => `${code} ×${count}`).join(', ')}
              </p>
            )}

            <p className="mt-3 text-meta text-aether-text-muted">
              Last review: {formatTime(stats.last_review_at)} · Next due:{' '}
              {formatTime(stats.next_review_due)}
            </p>

            <div className="mt-3 space-y-2 border-t border-aether-border pt-3">
              <div className="grid gap-2 sm:grid-cols-[10rem_1fr]">
                <input
                  value={reviewer}
                  onChange={(event) => setReviewer(event.target.value)}
                  aria-label="Reviewer"
                  placeholder="Reviewer"
                  className="rounded border border-aether-border bg-aether-bg px-3 py-1.5 text-meta text-aether-text outline-none focus:border-aether-primary"
                />
                <input
                  value={reviewNote}
                  onChange={(event) => setReviewNote(event.target.value)}
                  aria-label="Review note"
                  placeholder="What did you review? Any follow-up?"
                  className="rounded border border-aether-border bg-aether-bg px-3 py-1.5 text-meta text-aether-text outline-none focus:border-aether-primary"
                />
              </div>
              <button
                onClick={recordReview}
                disabled={recording || !reviewer.trim()}
                className="rounded border border-aether-border px-3 py-1.5 text-meta text-aether-text transition hover:bg-aether-primary/10 disabled:opacity-50"
              >
                {recording ? 'Recording…' : 'Record review'}
              </button>
              {reviewMessage && (
                <p className="text-meta text-aether-text-muted">{reviewMessage}</p>
              )}
            </div>
          </>
        )}
      </section>

      <p className="text-xs text-aether-text-muted">
        Veridicus never sends your resume, claims or applications to a remote service. Generation
        runs against Ollama on this machine.
      </p>
    </div>
  );
}
