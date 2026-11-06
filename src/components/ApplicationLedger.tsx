import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FolderOpen, LayoutGrid, List, Plus, X } from 'lucide-react';
import Dialog from './Dialog';
import { pushToast } from './Toaster';

interface ApplicationRecord {
  id: string;
  employer: string;
  role: string;
  req_id: string;
  date: string;
  base_resume_version: string;
  materials_version: string;
  claims_used: string;
  gaps_named: string;
  approved_wording: string;
  salary_entered: string | null;
  status: string;
  notes: string;
}

type StatusKey = 'PENDING_REVIEW' | 'SUBMITTED' | 'INTERVIEWING' | 'OFFER' | 'REJECTED';

const PIPELINE: { key: StatusKey; label: string; dot: string }[] = [
  { key: 'PENDING_REVIEW', label: 'Saved', dot: 'bg-aether-warning' },
  { key: 'SUBMITTED', label: 'Applied', dot: 'bg-aether-info' },
  { key: 'INTERVIEWING', label: 'Interviewing', dot: 'bg-aether-primary' },
  { key: 'OFFER', label: 'Offer', dot: 'bg-aether-success' },
  { key: 'REJECTED', label: 'Rejected', dot: 'bg-aether-error' },
];

const STATUS_SELECT_STYLES: Record<string, string> = {
  PENDING_REVIEW: 'bg-aether-warning text-aether-primary-fg',
  SUBMITTED: 'bg-aether-info text-aether-text',
  INTERVIEWING: 'bg-aether-primary text-aether-primary-fg',
  OFFER: 'bg-aether-success text-aether-primary-fg',
  REJECTED: 'bg-aether-error text-aether-primary-fg',
};

function labelFor(status: string) {
  return PIPELINE.find((stage) => stage.key === status)?.label ?? status;
}

function daysSince(date: string) {
  const parsed = Date.parse(date);
  if (Number.isNaN(parsed)) return 0;
  return (Date.now() - parsed) / 86_400_000;
}

function needsFollowUp(record: ApplicationRecord) {
  if (record.status !== 'SUBMITTED' && record.status !== 'INTERVIEWING') return false;
  return daysSince(record.date) >= 7;
}

const EXAMPLE_RECORD: ApplicationRecord = {
  id: 'example-not-saved',
  employer: 'Northwind Analytics',
  role: 'Director of Enablement',
  req_id: 'REQ-1042',
  date: new Date().toISOString().slice(0, 10),
  base_resume_version: 'master_v1',
  materials_version: 'master_v1',
  claims_used: '6',
  gaps_named: '2',
  approved_wording: '',
  salary_entered: null,
  status: 'SUBMITTED',
  notes: 'Example row — it is not stored anywhere and disappears when you hide it.',
};

export default function ApplicationLedger({ onStartApply }: { onStartApply?: () => void }) {
  const [records, setRecords] = useState<ApplicationRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<'board' | 'table'>('board');
  const [followUpOnly, setFollowUpOnly] = useState(false);
  const [showExample, setShowExample] = useState(true);
  const [selected, setSelected] = useState<ApplicationRecord | null>(null);
  const [cursor, setCursor] = useState(0);
  const [folderError, setFolderError] = useState<string | null>(null);
  const [sort, setSort] = useState<{
    key: 'date' | 'employer' | 'role' | 'status';
    direction: 'asc' | 'desc';
  }>({ key: 'date', direction: 'desc' });

  useEffect(() => {
    void loadRecords();
  }, []);

  const loadRecords = async () => {
    try {
      const data: ApplicationRecord[] = await invoke('get_application_records');
      setRecords(data);
      setError(null);
    } catch (err: any) {
      const message = err?.message || String(err);
      setError(message);
      pushToast({ kind: 'error', message: 'Could not load applications', detail: message });
    } finally {
      setLoading(false);
    }
  };

  const setStatus = async (id: string, status: string) => {
    const previous = records.find((record) => record.id === id)?.status;
    if (!previous || previous === status) return;

    setRecords((current) =>
      current.map((record) => (record.id === id ? { ...record, status } : record)),
    );
    setSelected((current) => (current && current.id === id ? { ...current, status } : current));

    try {
      await invoke('update_application_status', { id, status });
      pushToast({
        kind: 'success',
        message: `Moved to ${labelFor(status)}`,
        action: { label: 'Undo', onClick: () => void setStatus(id, previous) },
      });
    } catch (err: any) {
      const message = err?.message || String(err);
      setRecords((current) =>
        current.map((record) => (record.id === id ? { ...record, status: previous } : record)),
      );
      pushToast({
        kind: 'error',
        message: 'Status change was not saved',
        detail: message,
        action: { label: 'Retry', onClick: () => void setStatus(id, status) },
      });
    }
  };

  const openFolder = async (record: ApplicationRecord) => {
    setFolderError(null);
    try {
      const folder = await invoke<string>('get_application_folder', {
        applicationId: record.id,
      });
      await invoke('open_application_folder', { path: folder });
    } catch (err: any) {
      const message = err?.message || String(err);
      setFolderError(message);
      pushToast({ kind: 'error', message: 'Could not open the folder', detail: message });
    }
  };

  const visible = useMemo(() => {
    const filtered = followUpOnly ? records.filter(needsFollowUp) : records;
    const direction = sort.direction === 'asc' ? 1 : -1;
    return [...filtered].sort((a, b) => a[sort.key].localeCompare(b[sort.key]) * direction);
  }, [records, followUpOnly, sort]);

  const counts = useMemo(() => {
    const map: Record<string, number> = {};
    for (const stage of PIPELINE) map[stage.key] = 0;
    for (const record of records) map[record.status] = (map[record.status] || 0) + 1;
    return map;
  }, [records]);

  const activeCursor = Math.min(cursor, Math.max(visible.length - 1, 0));

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target && ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)) return;
      if (event.metaKey || event.ctrlKey || event.altKey) return;

      if (selected) {
        if (event.key === 'Escape') {
          event.preventDefault();
          setSelected(null);
        }
        return;
      }

      if (event.key === 'j') {
        event.preventDefault();
        setCursor((current) => Math.min(current + 1, Math.max(visible.length - 1, 0)));
      } else if (event.key === 'k') {
        event.preventDefault();
        setCursor((current) => Math.max(current - 1, 0));
      } else if (event.key === 'Enter' && visible[activeCursor]) {
        event.preventDefault();
        setSelected(visible[activeCursor]);
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [selected, visible, activeCursor]);

  const toggleSort = (key: 'date' | 'employer' | 'role' | 'status') => {
    setSort((previous) =>
      previous.key === key
        ? { key, direction: previous.direction === 'asc' ? 'desc' : 'asc' }
        : { key, direction: 'asc' },
    );
  };

  const statusSelect = (record: ApplicationRecord) => (
    <select
      value={record.status}
      onChange={(event) => void setStatus(record.id, event.target.value)}
      aria-label={`Status for ${record.employer}`}
      className={`rounded border border-aether-border px-2 py-1 text-meta font-medium ${
        STATUS_SELECT_STYLES[record.status] || 'bg-aether-bg text-aether-text'
      }`}
    >
      {PIPELINE.map((stage) => (
        <option key={stage.key} value={stage.key}>
          {stage.label}
        </option>
      ))}
    </select>
  );

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-section text-aether-text">Applications</h2>
          <p className="text-meta text-aether-text-muted">
            {records.length === 0
              ? 'Nothing tracked yet'
              : `${records.length} tracked · ${counts.SUBMITTED + counts.INTERVIEWING} in play`}
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <button
            onClick={() => setFollowUpOnly((previous) => !previous)}
            aria-pressed={followUpOnly}
            className={`rounded-full border px-3 py-1 text-meta transition ${
              followUpOnly
                ? 'border-aether-warning bg-aether-warning text-aether-primary-fg'
                : 'border-aether-border text-aether-text-muted hover:text-aether-text'
            }`}
          >
            Needs follow-up
          </button>
          <div className="flex overflow-hidden rounded-lg border border-aether-border">
            <button
              onClick={() => setMode('board')}
              aria-label="Board view"
              aria-pressed={mode === 'board'}
              className={`px-2 py-1.5 ${mode === 'board' ? 'bg-aether-primary/15 text-aether-text' : 'text-aether-text-muted'}`}
            >
              <LayoutGrid size={14} />
            </button>
            <button
              onClick={() => setMode('table')}
              aria-label="Table view"
              aria-pressed={mode === 'table'}
              className={`px-2 py-1.5 ${mode === 'table' ? 'bg-aether-primary/15 text-aether-text' : 'text-aether-text-muted'}`}
            >
              <List size={14} />
            </button>
          </div>
        </div>
      </div>

      {error && <p className="text-body text-aether-error">{error}</p>}

      {loading ? (
        <p className="animate-pulse text-body text-aether-text-muted">Loading applications…</p>
      ) : records.length === 0 ? (
        <div className="rounded-xl border border-dashed border-aether-border bg-aether-surface p-6">
          <div className="flex items-start justify-between gap-3">
            <div>
              <h3 className="text-row font-semibold text-aether-text">
                Here is what a tracked application looks like
              </h3>
              <p className="mt-1 text-meta text-aether-text-muted">
                This example is not saved anywhere. Run an application to replace it with real
                data.
              </p>
            </div>
            {showExample && (
              <button
                onClick={() => setShowExample(false)}
                className="text-aether-text-muted transition hover:text-aether-text"
                aria-label="Hide example"
              >
                <X size={14} />
              </button>
            )}
          </div>

          {showExample && (
            <div className="mt-4 grid gap-3 rounded-lg border border-aether-border bg-aether-bg p-4 md:grid-cols-[1fr_1fr_auto]">
              <div>
                <p className="text-body text-aether-text">{EXAMPLE_RECORD.employer}</p>
                <p className="text-meta text-aether-text-muted">{EXAMPLE_RECORD.role}</p>
              </div>
              <div className="flex flex-wrap items-center gap-2 text-meta text-aether-text-muted">
                <span className="rounded-full border border-aether-border px-2 py-0.5">
                  Example · not saved
                </span>
                <span>{EXAMPLE_RECORD.claims_used} claims used</span>
                <span>{EXAMPLE_RECORD.gaps_named} gaps named</span>
              </div>
              <div className="flex items-center gap-2">{statusSelect(EXAMPLE_RECORD)}</div>
            </div>
          )}

          {onStartApply && (
            <button
              onClick={onStartApply}
              className="mt-4 flex items-center gap-2 rounded bg-aether-primary px-4 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover"
            >
              <Plus size={16} /> Run your first application
            </button>
          )}
        </div>
      ) : mode === 'board' ? (
        <div className="grid gap-3 lg:grid-cols-5">
          {PIPELINE.map((stage) => {
            const columnRecords = visible.filter((record) => record.status === stage.key);
            return (
              <section
                key={stage.key}
                className="flex min-h-[140px] flex-col gap-2 rounded-xl border border-aether-border bg-aether-surface p-3"
              >
                <header className="flex items-center gap-2 text-meta font-semibold text-aether-text-muted">
                  <span className={`h-2 w-2 rounded-full ${stage.dot}`} />
                  {stage.label}
                  <span className="ml-auto rounded-full border border-aether-border px-1.5">
                    {counts[stage.key] || 0}
                  </span>
                </header>

                {columnRecords.length === 0 && (
                  <p className="text-meta text-aether-text-muted">Nothing here yet.</p>
                )}

                {columnRecords.map((record) => {
                  const isCursor = visible[activeCursor]?.id === record.id;
                  return (
                    <div
                      key={record.id}
                      className={`rounded-lg border bg-aether-bg p-2 transition ${
                        isCursor ? 'border-aether-primary' : 'border-aether-border'
                      }`}
                    >
                      <button
                        onClick={() => setSelected(record)}
                        className="w-full text-left"
                      >
                        <p className="truncate text-body text-aether-text">{record.employer}</p>
                        <p className="truncate text-meta text-aether-text-muted">{record.role}</p>
                        <p className="mt-1 text-meta text-aether-text-muted">{record.date}</p>
                      </button>
                      <div className="mt-2">{statusSelect(record)}</div>
                    </div>
                  );
                })}
              </section>
            );
          })}
        </div>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-aether-border bg-aether-surface">
          <table className="w-full border-collapse text-left">
            <thead>
              <tr className="border-b border-aether-border">
                {(
                  [
                    ['employer', 'Employer'],
                    ['role', 'Role'],
                    ['date', 'Date'],
                    ['status', 'Status'],
                  ] as const
                ).map(([key, label]) => (
                  <th key={key} className="px-4 py-3">
                    <button
                      onClick={() => toggleSort(key)}
                      className="text-meta font-semibold uppercase tracking-wider text-aether-text-muted transition hover:text-aether-text"
                    >
                      {label}
                      {sort.key === key ? (sort.direction === 'asc' ? ' ↑' : ' ↓') : ''}
                    </button>
                  </th>
                ))}
                <th className="px-4 py-3 text-meta font-semibold uppercase tracking-wider text-aether-text-muted">
                  Claims / Gaps
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-aether-border">
              {visible.map((record, index) => (
                <tr
                  key={record.id}
                  onClick={() => setSelected(record)}
                  className={`cursor-pointer transition ${
                    index === activeCursor ? 'bg-aether-primary/10' : 'hover:bg-aether-bg/60'
                  }`}
                >
                  <td className="px-4 py-3 text-body text-aether-text">{record.employer}</td>
                  <td className="px-4 py-3 text-body text-aether-text">{record.role}</td>
                  <td className="px-4 py-3 text-body text-aether-text-muted">{record.date}</td>
                  <td className="px-4 py-3" onClick={(event) => event.stopPropagation()}>
                    {statusSelect(record)}
                  </td>
                  <td className="px-4 py-3 text-body text-aether-text-muted">
                    {record.claims_used || '0'} / {record.gaps_named || '0'}
                  </td>
                </tr>
              ))}
              {visible.length === 0 && (
                <tr>
                  <td colSpan={5} className="px-4 py-8 text-center text-body text-aether-text-muted">
                    No applications match this filter.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      <p className="text-meta text-aether-text-muted">
        j / k to move, Enter to open, ? for all shortcuts.
      </p>

      <Dialog
        isOpen={selected !== null}
        onClose={() => setSelected(null)}
        align="sheet"
        title={selected?.employer ?? ''}
        description={selected?.role}
      >
        {selected && (
          <div className="space-y-4">
            <div className="flex flex-wrap items-center gap-2">
              {statusSelect(selected)}
              <button
                onClick={() => void openFolder(selected)}
                className="flex items-center gap-2 rounded border border-aether-border px-3 py-1.5 text-meta text-aether-text transition hover:bg-aether-bg"
              >
                <FolderOpen size={14} /> Open folder
              </button>
            </div>
            {folderError && <p className="text-meta text-aether-error">{folderError}</p>}

            <dl className="grid grid-cols-2 gap-3">
              {[
                ['Applied', selected.date],
                ['Req ID', selected.req_id || '—'],
                ['Claims used', selected.claims_used || '0'],
                ['Gaps named', selected.gaps_named || '0'],
                ['Base resume', selected.base_resume_version || '—'],
                ['Materials', selected.materials_version || '—'],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="rounded-lg border border-aether-border bg-aether-bg p-2"
                >
                  <dt className="text-meta text-aether-text-muted">{label}</dt>
                  <dd className="mt-0.5 text-meta text-aether-text">{value}</dd>
                </div>
              ))}
            </dl>

            <div>
              <p className="mb-1 text-meta text-aether-text-muted">Notes</p>
              <p className="whitespace-pre-wrap rounded-lg border border-aether-border bg-aether-bg p-3 text-meta text-aether-text">
                {selected.notes || 'No notes recorded.'}
              </p>
            </div>

            <p className="break-all font-mono text-meta text-aether-text-muted">{selected.id}</p>
          </div>
        )}
      </Dialog>
    </div>
  );
}
