interface DashboardViewProps {
  resumeStatus: string;
  onStartApply: () => void;
  onOpenApplications: () => void;
  onOpenPrep: () => void;
  onOpenDocuments: () => void;
}

export default function DashboardView({
  resumeStatus,
  onStartApply,
  onOpenApplications,
  onOpenPrep,
  onOpenDocuments,
}: DashboardViewProps) {
  return (
    <div className="max-w-4xl space-y-5">
      <div>
        <h2 className="text-xl font-bold text-aether-text">Dashboard</h2>
        <p className="text-sm text-aether-text-muted">
          Everything runs on this machine. No account, no upload.
        </p>
      </div>

      <div className="grid gap-3 md:grid-cols-2">
        <section className="rounded-xl border border-aether-border bg-aether-surface p-5">
          <h3 className="text-sm font-semibold text-aether-text">Apply to a posting</h3>
          <p className="mt-1 text-xs text-aether-text-muted">
            Paste a job URL. CareerOS scrapes it, tailors your resume and letter, then fact-checks
            both before anything is committed.
          </p>
          <code className="mt-3 block truncate rounded bg-aether-bg px-2 py-1 font-mono text-xs text-aether-text-muted">
            /apply https://boards.greenhouse.io/acme/jobs/123
          </code>
          <button
            onClick={onStartApply}
            className="mt-3 rounded bg-aether-primary px-3 py-1.5 text-xs font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover"
          >
            Start an application
          </button>
        </section>

        <section className="rounded-xl border border-aether-border bg-aether-surface p-5">
          <h3 className="text-sm font-semibold text-aether-text">Track what you sent</h3>
          <p className="mt-1 text-xs text-aether-text-muted">
            Saved, applied, interviewing, offer, rejected — move each application through the
            pipeline and see who needs a follow-up.
          </p>
          <button
            onClick={onOpenApplications}
            className="mt-3 rounded border border-aether-border px-3 py-1.5 text-xs text-aether-text transition hover:bg-aether-bg"
          >
            Open applications
          </button>
        </section>

        <section className="rounded-xl border border-aether-border bg-aether-surface p-5">
          <h3 className="text-sm font-semibold text-aether-text">Prepare for the call</h3>
          <p className="mt-1 text-xs text-aether-text-muted">
            Generate a company briefing with talking points, or run a STAR-method mock interview.
          </p>
          <button
            onClick={onOpenPrep}
            className="mt-3 rounded border border-aether-border px-3 py-1.5 text-xs text-aether-text transition hover:bg-aether-bg"
          >
            Open prep
          </button>
        </section>

        <section className="rounded-xl border border-aether-border bg-aether-surface p-5">
          <h3 className="text-sm font-semibold text-aether-text">Keep your master resume</h3>
          <p className="mt-1 text-xs text-aether-text-muted">
            Import a DOCX, PDF or Markdown resume so claims are extracted once and reused
            everywhere.
          </p>
          <button
            onClick={onOpenDocuments}
            className="mt-3 rounded border border-aether-border px-3 py-1.5 text-xs text-aether-text transition hover:bg-aether-bg"
          >
            Open documents
          </button>
        </section>
      </div>

      {resumeStatus && (
        <div className="rounded-lg border border-aether-border bg-aether-surface p-3 text-sm">
          <p className={resumeStatus.startsWith('✓') ? 'text-aether-success' : 'text-aether-error'}>
            {resumeStatus}
          </p>
        </div>
      )}
    </div>
  );
}
