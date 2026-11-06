import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

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
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [isOpen, initialFormat]);

  const exportResume = async () => {
    if (!markdown.trim()) {
      setStatus('Nothing to export. Import a resume with /resume first.');
      return;
    }

    setLoading(true);
    setStatus('Checking ATS compliance...');

    try {
      const report: any = await invoke('check_ats_compliance', { markdown });
      const warnings: string[] = report.suggestions || [];

      setStatus('Exporting...');
      const result: any = await invoke('export_ats_resume', {
        markdown,
        format,
        outputDir: null,
      });

      if (result.success) {
        const atsNote = report.passed ? '' : ` ATS warnings: ${warnings.join('; ')}`;
        setStatus(`✓ Exported ${format.toUpperCase()}: ${result.output_path}${atsNote}`);
      } else {
        setStatus(`✗ Error: ${result.error_message}`);
      }
    } catch (err: any) {
      setStatus(`Error: ${err.message || String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  if (!isOpen) {
    return null;
  }

  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 z-50">
      <div className="bg-aether-surface border border-aether-border rounded-xl w-full max-w-2xl overflow-hidden shadow-2xl">
        <div className="p-4 border-b border-aether-border flex items-center justify-between">
          <h3 className="text-lg font-bold text-aether-text">Export ATS Resume</h3>
          <button
            onClick={onClose}
            className="text-aether-text-muted hover:text-aether-text"
          >
            ✕
          </button>
        </div>
        <div className="p-4">
          <div className="mb-4">
            <label className="block text-sm text-aether-text-muted mb-2">Format</label>
            <div className="flex gap-2">
              {formats.map((fmt) => (
                <button
                  key={fmt}
                  onClick={() => setFormat(fmt)}
                  className={`px-3 py-1 rounded text-xs border transition ${
                    format === fmt
                      ? 'bg-aether-primary text-aether-primary-fg border-aether-primary'
                      : 'bg-aether-bg border-aether-border text-aether-text'
                  }`}
                >
                  {fmt.toUpperCase()}
                </button>
              ))}
            </div>
          </div>
          <div className="mb-4">
            <label className="block text-sm text-aether-text-muted mb-2">Resume Markdown</label>
            <textarea
              value={markdown}
              onChange={(e) => setMarkdown(e.target.value)}
              rows={10}
              placeholder="Import a resume with /resume to load it here."
              className="w-full bg-aether-bg border border-aether-border text-aether-text p-2 text-sm font-mono"
            />
          </div>
          <div className="flex gap-2">
            <button
              onClick={exportResume}
              disabled={loading}
              className="flex-1 py-2 rounded bg-aether-primary text-aether-primary-fg hover:bg-aether-primary-hover font-medium disabled:opacity-50 transition"
            >
              {loading ? 'Exporting...' : 'Export Resume'}
            </button>
            <button
              onClick={onClose}
              className="px-4 py-2 rounded bg-aether-bg border border-aether-border text-aether-text hover:bg-aether-bg/50 transition"
            >
              Cancel
            </button>
          </div>
          {status && (
            <div className="mt-4 p-3 rounded bg-aether-bg border border-aether-border text-sm">
              <p className={`whitespace-pre-wrap ${status.startsWith('✓') ? 'text-aether-success' : 'text-aether-error'}`}>
                {status}
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
