import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import Markdown from './Markdown';

interface BriefingViewerProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  markdown: string;
}

type Format = 'txt' | 'md' | 'docx' | 'odt' | 'pdf';

export default function BriefingViewer({ isOpen, onClose, title, markdown }: BriefingViewerProps) {
  const [format, setFormat] = useState<Format>('md');
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<string>('');

  const formats: Format[] = ['txt', 'md', 'docx', 'odt', 'pdf'];

  const formatDate = () => {
    const d = new Date();
    return d.toISOString().replace(/[:.]/g, '-').slice(0, 19);
  };

  const safeName = (name: string) =>
    name.replace(/[^a-zA-Z0-9-_]+/g, '_').replace(/_+/g, '_').replace(/^_|_$/g, '') || 'Briefing';

  // The save dialog and the file write both run in Rust: the webview has no dialog or
  // filesystem plugin permissions, and it does not need them.
  const saveTxtMd = async (ext: 'txt' | 'md') => {
    const contents = ext === 'txt' ? markdown.replace(/[#*`_[\]]/g, '') : markdown;
    return invoke<string | null>('save_text_via_dialog', {
      defaultName: `${safeName(title)}_Prep_${formatDate()}.${ext}`,
      contents,
    });
  };

  const saveViaExport = async (fmt: 'docx' | 'odt' | 'pdf') => {
    const result: any = await invoke('export_ats_resume', {
      markdown,
      format: fmt,
      outputDir: '',
    });
    if (!result.success) {
      throw new Error(result.error_message || 'Export failed');
    }
    return result.output_path as string;
  };

  const handleSave = async () => {
    setLoading(true);
    setStatus('');
    try {
      const path =
        format === 'txt' || format === 'md' ? await saveTxtMd(format) : await saveViaExport(format);
      if (path) {
        setStatus(`✓ Saved: ${path}`);
      } else {
        setStatus('Save cancelled.');
      }
    } catch (err: any) {
      setStatus(`✗ Error: ${err.message || String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm">
      <div className="flex max-h-[90vh] w-full max-w-4xl flex-col overflow-hidden rounded-xl border border-aether-border bg-aether-surface shadow-2xl">
        <div className="flex items-center justify-between border-b border-aether-border p-4">
          <h3 className="text-lg font-bold text-aether-text">
            Company briefing — {title || 'Unknown'}
          </h3>
          <button
            onClick={onClose}
            className="text-aether-text-muted transition hover:text-aether-text"
            aria-label="Close briefing"
          >
            ✕
          </button>
        </div>

        <div className="flex-1 overflow-auto p-6">
          <Markdown content={markdown} />
        </div>

        <div className="flex flex-wrap items-center gap-3 border-t border-aether-border p-4">
          <label className="text-sm text-aether-text-muted">Format:</label>
          <div className="flex gap-2">
            {formats.map((fmt) => (
              <button
                key={fmt}
                onClick={() => setFormat(fmt)}
                className={`rounded border px-3 py-1 text-xs transition ${
                  format === fmt
                    ? 'border-aether-primary bg-aether-primary text-aether-primary-fg'
                    : 'border-aether-border bg-aether-bg text-aether-text'
                }`}
              >
                {fmt.toUpperCase()}
              </button>
            ))}
          </div>
          <div className="flex-1" />
          <button
            onClick={handleSave}
            disabled={loading}
            className="rounded bg-aether-primary px-4 py-2 text-sm font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
          >
            {loading ? 'Saving...' : 'Save As...'}
          </button>
          <button
            onClick={onClose}
            className="rounded border border-aether-border bg-aether-bg px-4 py-2 text-sm text-aether-text transition hover:bg-aether-surface"
          >
            Close
          </button>
        </div>

        {status && (
          <div className="px-4 pb-4">
            <div className="rounded border border-aether-border bg-aether-bg p-2 text-xs">
              <p
                className={
                  status.startsWith('\u2713')
                    ? 'text-aether-success'
                    : status.startsWith('\u2717')
                      ? 'text-aether-error'
                      : 'text-aether-text'
                }
              >
                {status}
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
