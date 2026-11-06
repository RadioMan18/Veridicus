import { Download, Upload } from 'lucide-react';

interface DocumentsViewProps {
  workspacePath: string;
  gitActive: boolean;
  resumeStatus: string;
  busy: boolean;
  onImportResume: () => void;
  onExport: (format: 'pdf' | 'docx' | 'odt') => void;
}

const EXPORT_FORMATS = ['pdf', 'docx', 'odt'] as const;

export default function DocumentsView({
  workspacePath,
  gitActive,
  resumeStatus,
  busy,
  onImportResume,
  onExport,
}: DocumentsViewProps) {
  return (
    <div className="max-w-3xl space-y-5">
      <div>
        <h2 className="text-section text-aether-text">Documents</h2>
        <p className="text-body text-aether-text-muted">
          Your master resume is the source for every tailored application.
        </p>
      </div>

      <section className="rounded-xl border border-aether-border bg-aether-surface p-5">
        <h3 className="text-sm font-semibold text-aether-text">Master resume</h3>
        <div className="mt-3 flex flex-wrap gap-2">
          <button
            onClick={onImportResume}
            disabled={busy}
            className="flex items-center gap-2 rounded bg-aether-primary px-3 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
          >
            <Upload size={14} /> Import resume
          </button>
          {EXPORT_FORMATS.map((format) => (
            <button
              key={format}
              onClick={() => onExport(format)}
              className="flex items-center gap-2 rounded border border-aether-border px-3 py-2 text-body text-aether-text transition hover:bg-aether-bg"
            >
              <Download size={14} /> {format.toUpperCase()}
            </button>
          ))}
        </div>
        {resumeStatus && (
          <p
            className={`mt-3 text-xs ${
              resumeStatus.startsWith('✓') ? 'text-aether-success' : 'text-aether-error'
            }`}
          >
            {resumeStatus}
          </p>
        )}
      </section>

      <section className="rounded-xl border border-aether-border bg-aether-surface p-5">
        <h3 className="text-sm font-semibold text-aether-text">Where files live</h3>
        <p className="mt-2 break-all font-mono text-xs text-aether-text-muted">
          {workspacePath || 'Loading workspace path…'}
        </p>
        <p className="mt-2 text-xs text-aether-text-muted">
          Generated resumes, letters and QA page renders are written to{' '}
          <span className="font-mono">applications/&lt;id&gt;/</span>.{' '}
          {gitActive ? 'Local git history is active.' : 'No local git history yet.'}
        </p>
      </section>
    </div>
  );
}
