import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import Markdown from './Markdown';

export default function PrepView() {
  const [company, setCompany] = useState('');
  const [output, setOutput] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    const name = company.trim();
    if (!name) {
      setError('Enter a company name first.');
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const briefing: string = await invoke('run_company_research', { company: name });
      setOutput(briefing);
    } catch (err: any) {
      setError(err?.message || String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="max-w-3xl space-y-4">
      <div>
        <h2 className="text-section text-aether-text">Prep</h2>
        <p className="text-body text-aether-text-muted">
          Research the company before the first call. Generated locally.
        </p>
      </div>

      <div className="flex flex-wrap gap-2">
        <input
          value={company}
          onChange={(event) => setCompany(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') void run();
          }}
          placeholder="Company name, e.g. NetDocuments"
          className="min-w-[240px] flex-1 rounded-lg border border-aether-border bg-aether-surface px-3 py-2 text-sm outline-none focus:border-aether-primary"
        />
        <button
          onClick={run}
          disabled={loading}
          className="rounded-lg bg-aether-primary px-4 py-2 text-sm font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
        >
          {loading ? 'Researching…' : 'Research'}
        </button>
      </div>

      {error && <p className="text-body text-aether-error">{error}</p>}

      {output ? (
        <section className="rounded-xl border border-aether-border bg-aether-surface p-6">
          <Markdown content={output} />
        </section>
      ) : (
        <p className="text-body text-aether-text-muted">
          Nothing yet. Enter a company to generate a briefing with recent context, likely interview
          themes and talking points.
        </p>
      )}
    </div>
  );
}
