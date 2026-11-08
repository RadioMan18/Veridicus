import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import RoleFit, { type PersistedGap, type RoleFitAnalysis } from './RoleFit';

interface ApplicationRecord {
  id: string;
  employer: string;
  role: string;
  date: string;
  status: string;
}

export default function FitView({ onStartApply }: { onStartApply: () => void }) {
  const [records, setRecords] = useState<ApplicationRecord[]>([]);
  const [selected, setSelected] = useState('');
  const [analysis, setAnalysis] = useState<RoleFitAnalysis | null>(null);
  const [gaps, setGaps] = useState<PersistedGap[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    invoke<ApplicationRecord[]>('get_application_records')
      .then((data) => {
        if (!active) return;
        setRecords(data);
        if (data.length > 0) setSelected(data[0].id);
      })
      .catch((err: any) => {
        if (active) setError(err?.message || String(err));
      });
    return () => {
      active = false;
    };
  }, []);

  const load = async (applicationId: string) => {
    setLoading(true);
    setError(null);
    try {
      const [matrix, rows] = await Promise.all([
        invoke<RoleFitAnalysis>('get_fit_matrix', { applicationId }).catch(() => null),
        invoke<PersistedGap[]>('list_gaps', { applicationId }),
      ]);
      setAnalysis(matrix);
      setGaps(rows);
    } catch (err: any) {
      setError(err?.message || String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (selected) void load(selected);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected]);

  return (
    <div className="max-w-4xl space-y-4">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 className="text-section text-aether-text">Role fit</h2>
          <p className="text-body text-aether-text-muted">
            What the posting asks for, what your ledger proves, and the gaps between them.
          </p>
        </div>
        <button
          onClick={onStartApply}
          className="rounded-lg bg-aether-primary px-4 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover"
        >
          New application
        </button>
      </div>

      {records.length === 0 ? (
        <section className="rounded-xl border border-aether-border bg-aether-surface p-6">
          <p className="text-body text-aether-text-muted">
            No applications yet. Run /apply on a posting and its fit matrix will appear here.
          </p>
        </section>
      ) : (
        <>
          <label className="block max-w-md text-body text-aether-text">
            Application
            <select
              value={selected}
              onChange={(event) => setSelected(event.target.value)}
              className="mt-1 w-full rounded-lg border border-aether-border bg-aether-surface px-3 py-2 text-body text-aether-text"
            >
              {records.map((record) => (
                <option key={record.id} value={record.id}>
                  {record.date} · {record.employer} — {record.role}
                </option>
              ))}
            </select>
          </label>

          <section className="rounded-xl border border-aether-border bg-aether-surface p-4">
            {loading && <p className="text-meta text-aether-text-muted">Loading the matrix…</p>}
            {error && <p className="text-meta text-aether-error">{error}</p>}
            {!loading && (
              <RoleFit
                analysis={analysis}
                gaps={gaps}
                onDecision={() => selected && void load(selected)}
              />
            )}
          </section>
        </>
      )}
    </div>
  );
}
