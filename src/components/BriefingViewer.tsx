import { useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import { invoke } from '@tauri-apps/api/core';

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

  const safeName = (name: string) => {
    return name.replace(/[^a-zA-Z0-9-_]+/g, '_').replace(/_+/g, '_').replace(/^_|_$/g, '') || 'Briefing';
  };

  const saveTxtMd = async (ext: 'txt' | 'md') => {
    const defaultName = `${safeName(title)}_Prep_${formatDate()}.${ext}`;
    const filePath = await save({
      defaultPath: defaultName,
      filters: [
        {
          name: ext.toUpperCase(),
          extensions: [ext],
        },
      ],
    });
    if (!filePath) return null;
    if (ext === 'txt') {
      const plain = markdown.replace(/[#*`_\[\]]/g, '');
      await writeTextFile(filePath, plain);
    } else {
      await writeTextFile(filePath, markdown);
    }
    return filePath;
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
      let path: string | null = null;
      if (format === 'txt') {
        path = await saveTxtMd('txt');
      } else if (format === 'md') {
        path = await saveTxtMd('md');
      } else if (format === 'docx') {
        path = await saveViaExport('docx');
      } else if (format === 'odt') {
        path = await saveViaExport('odt');
      } else if (format === 'pdf') {
        path = await saveViaExport('pdf');
      }
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
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 z-50">
      <div className="bg-aether-surface border border-aether-border rounded-xl w-full max-w-4xl overflow-hidden shadow-2xl max-h-[90vh] flex flex-col">
        <div className="p-4 border-b border-aether-border flex items-center justify-between">
          <h3 className="text-lg font-bold text-aether-text">Company Briefing - {title || 'Unknown'}</h3>
          <button onClick={onClose} className="text-aether-text-muted hover:text-aether-text">✕</button>
        </div>
        <div className="p-4 flex-1 overflow-auto">
          <pre className="whitespace-pre-wrap font-mono text-sm text-aether-text bg-aether-bg border border-aether-border rounded p-4 overflow-x-auto">{markdown}</pre>
        </div>
        <div className="p-4 border-t border-aether-border flex items-center gap-3">
          <label className="text-sm text-aether-text-muted">Format:</label>
          <div className="flex gap-2">
            {formats.map((fmt) => (
              <button
                key={fmt}
                onClick={() => setFormat(fmt)}
                className={`px-3 py-1 rounded text-xs border transition ${format === fmt ? 'bg-aether-primary text-aether-primary-fg border-aether-primary' : 'bg-aether-bg border-aether-border text-aether-text'}`}
              >
                {fmt.toUpperCase()}
              </button>
            ))}
          </div>
          <div className="flex-1" />
          <button onClick={handleSave} disabled={loading} className="px-4 py-2 rounded bg-aether-primary text-aether-primary-fg hover:bg-aether-primary-hover font-medium disabled:opacity-50 transition">
            {loading ? 'Saving...' : 'Save As...'}
          </button>
          <button onClick={onClose} className="px-4 py-2 rounded bg-aether-bg border border-aether-border text-aether-text hover:bg-aether-bg/50 transition">Close</button>
        </div>
        {status && (
          <div className="px-4 pb-4">
            <div className="p-2 rounded bg-aether-bg border border-aether-border text-xs">
              <p className={status.startsWith('\u2713') ? 'text-aether-success' : status.startsWith('\u2717') ? 'text-aether-error' : 'text-aether-text'}>{status}</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
