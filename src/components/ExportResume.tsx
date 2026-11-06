import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import Dialog from './Dialog';
import { pushToast } from './Toaster';

interface ExportResumeProps {
  isOpen: boolean;
  initialFormat: 'pdf' | 'docx' | 'odt';
  onClose: () => void;
}

const FALLBACK_MARKDOWN =
  '# Your Name\n\n## Summary\n\n- Import your resume with /resume to edit and export it here.\n';

export default function ExportResume({ isOpen, initialFormat, onClose }: ExportResumeProps) {
  const [format, setFormat] = useState<'pdf' | 'docx' | 'odt'>('pdf');
  const [status, setStatus] = useState<string>('');
  const [statusKind, setStatusKind] = useState<'ok' | 'error' | 'info'>('info');
  const [loading, setLoading] = useState(false);
  const [markdown, setMarkdown] = useState('');

  const formats = ['pdf', 'docx', 'odt'] as const;

  useEffect(() => {
    if (!isOpen) return;
    setFormat(initialFormat);
    setStatus('');
    let cancelled = false;

    (async () => {
      try {
        const master: string = await invoke('get_master_resume');
        if (!cancelled) setMarkdown(master);
      } catch (err: any) {
        if (!cancelled) {
          setMarkdown(FALLBACK_MARKDOWN);
          setStatus(`Could not load master resume: ${err.message || String(err)}`);
          setStatusKind('error');
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [isOpen, initialFormat]);

  const exportResume = async () => {
    if (!markdown.trim()) {
      setStatus('Nothing to export. Import a resume first, then export.');
      setStatusKind('error');
      return;
    }

    setLoading(true);
    setStatus('Checking ATS compliance…');
    setStatusKind('info');

    try {
      const report: any = await invoke('check_ats_compliance', { markdown });
      const warnings: string[] = report.suggestions || [];

      setStatus('Exporting…');
      const result: any = await invoke('export_ats_resume', {
        markdown,
        format,
        outputDir: null,
      });

      if (result.success) {
        const atsNote = report.passed ? '' : ` ATS warnings: ${warnings.join('; ')}`;
        setStatus(`Exported ${format.toUpperCase()}${atsNote}`);
        setStatusKind(atsNote ? 'info' : 'ok');
        pushToast({
          kind: 'success',
          message: `Exported ${format.toUpperCase()} resume`,
          detail: result.output_path,
        });
      } else {
        setStatus(`Error: ${result.error_message}`);
        setStatusKind('error');
        pushToast({
          kind: 'error',
          message: `${format.toUpperCase()} export failed`,
          detail: result.error_message,
        });
      }
    } catch (err: any) {
      const message = err.message || String(err);
      setStatus(`Error: ${message}`);
      setStatusKind('error');
      pushToast({ kind: 'error', message: 'Export failed', detail: message });
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onClose}
      title="Export resume"
      description="ATS-safe PDF, DOCX or ODT built from your master resume."
      size="md"
      footer={
        <>
          <button
            onClick={onClose}
            className="rounded border border-aether-border bg-aether-bg px-4 py-2 text-body text-aether-text transition hover:bg-aether-surface"
          >
            Cancel
          </button>
          <button
            onClick={exportResume}
            disabled={loading}
            className="rounded bg-aether-primary px-4 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
          >
            {loading ? 'Exporting…' : 'Export resume'}
          </button>
        </>
      }
    >
      <div className="space-y-4">
        <div>
          <p className="mb-2 text-meta font-medium uppercase tracking-wider text-aether-text-muted">
            Format
          </p>
          <div className="flex gap-2">
            {formats.map((fmt) => (
              <button
                key={fmt}
                onClick={() => setFormat(fmt)}
                aria-pressed={format === fmt}
                className={`rounded border px-3 py-1 text-body transition ${
                  format === fmt
                    ? 'border-aether-primary bg-aether-primary text-aether-primary-fg'
                    : 'border-aether-border bg-aether-bg text-aether-text hover:border-aether-primary'
                }`}
              >
                {fmt.toUpperCase()}
              </button>
            ))}
          </div>
        </div>

        <div>
          <label
            htmlFor="resume-markdown"
            className="mb-2 block text-meta font-medium uppercase tracking-wider text-aether-text-muted"
          >
            Resume markdown
          </label>
          <textarea
            id="resume-markdown"
            value={markdown}
            onChange={(e) => setMarkdown(e.target.value)}
            rows={10}
            placeholder="Import a resume to load it here."
            className="w-full rounded border border-aether-border bg-aether-bg p-2 font-mono text-meta text-aether-text"
          />
        </div>

        {status && (
          <p
            role={statusKind === 'error' ? 'alert' : 'status'}
            className={`whitespace-pre-wrap rounded border border-aether-border bg-aether-bg p-3 text-body ${
              statusKind === 'ok'
                ? 'text-aether-success'
                : statusKind === 'error'
                  ? 'text-aether-error'
                  : 'text-aether-text'
            }`}
          >
            {status}
          </p>
        )}
      </div>
    </Dialog>
  );
}
