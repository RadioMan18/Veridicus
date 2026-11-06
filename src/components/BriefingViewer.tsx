import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import Dialog from './Dialog';
import Markdown from './Markdown';
import { pushToast } from './Toaster';

interface BriefingViewerProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  markdown: string;
}

type Format = 'txt' | 'md' | 'docx' | 'odt' | 'pdf';

const FORMATS: Format[] = ['txt', 'md', 'docx', 'odt', 'pdf'];

export default function BriefingViewer({ isOpen, onClose, title, markdown }: BriefingViewerProps) {
  const [format, setFormat] = useState<Format>('md');
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<string>('');
  const [statusKind, setStatusKind] = useState<'ok' | 'error' | 'info'>('info');

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
        setStatus(`Saved ${format.toUpperCase()}`);
        setStatusKind('ok');
        pushToast({ kind: 'success', message: `Briefing saved as ${format.toUpperCase()}`, detail: path });
      } else {
        setStatus('Save cancelled.');
        setStatusKind('info');
      }
    } catch (err: any) {
      const message = err.message || String(err);
      setStatus(`Error: ${message}`);
      setStatusKind('error');
      pushToast({ kind: 'error', message: 'Could not save briefing', detail: message });
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onClose}
      title={`Company briefing — ${title || 'Unknown'}`}
      description="Generated on this machine from public sources."
      size="lg"
      footer={
        <>
          <div className="mr-auto flex items-center gap-2">
            <span className="text-meta text-aether-text-muted">Save as</span>
            {FORMATS.map((fmt) => (
              <button
                key={fmt}
                onClick={() => setFormat(fmt)}
                aria-pressed={format === fmt}
                className={`rounded border px-2.5 py-1 text-meta transition ${
                  format === fmt
                    ? 'border-aether-primary bg-aether-primary text-aether-primary-fg'
                    : 'border-aether-border bg-aether-bg text-aether-text hover:border-aether-primary'
                }`}
              >
                {fmt.toUpperCase()}
              </button>
            ))}
          </div>
          <button
            onClick={handleSave}
            disabled={loading}
            className="rounded bg-aether-primary px-4 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
          >
            {loading ? 'Saving…' : 'Save'}
          </button>
        </>
      }
    >
      <Markdown content={markdown} />
      {status && (
        <p
          className={`mt-4 rounded border border-aether-border bg-aether-bg p-2 text-meta ${
            statusKind === 'ok'
              ? 'text-aether-success'
              : statusKind === 'error'
                ? 'text-aether-error'
                : 'text-aether-text-muted'
          }`}
        >
          {status}
        </p>
      )}
    </Dialog>
  );
}
