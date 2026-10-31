import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { saveAs } from 'file-saver';

interface ExportResumeProps {
  onClose: () => void;
}

export default function ExportResume({ onClose }: ExportResumeProps) {
  const [format, setFormat] = useState<'pdf' | 'docx' | 'odt'>('pdf');
  const [status, setStatus] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [markdown, setMarkdown] = useState('# Career Resume\n\n## Summary\n- Experienced professional\n\n## Experience\n- Professional experience\n\n## Skills\n- Professional skills\n\n## Education\n- Degree in relevant field\n');

  const formats = ['pdf', 'docx', 'odt'] as const;

  const exportResume = async () => {
    setLoading(true);
    setStatus('Generating ATS-optimized resume...');

    try {
      // First check ATS compliance
      const report: any = await invoke('check_ats_compliance', {
        markdown,
      });

      if (!report.passed) {
        setStatus(`⚠ ATS warnings: ${report.suggestions.join(', ')}. Proceeding anyway...`);
      }

      // Generate ATS-optimized markdown
      const optimizedMd: string = await invoke('generate_ats_resume_markdown', {
        claims: [],
        jdText: '',
        contactInfo: {
          name: '',
          phone: '',
          email: '',
          linkedIn: '',
        },
      });

      // Export to selected format
      const result: any = await invoke('export_ats_resume', {
        markdown: optimizedMd,
        format,
        outputDir: '',
      });

      if (result.success) {
        setStatus(`✓ Resume exported as ${format.toUpperCase()}: ${result.output_path}`);
      } else {
        setStatus(`✗ Error: ${result.error_message}`);
      }
    } catch (err: any) {
      setStatus(`Error: ${err.message || String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  if (!status) {
    onClose();
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
            <label className="block text-sm text-aether-text-muted mb-2">Markdown Content</label>
            <textarea
              value={markdown}
              onChange={(e) => setMarkdown(e.target.value)}
              rows={6}
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
              <p className={status.startsWith('✓') ? 'text-aether-success' : 'text-aether-error'}>
                {status}
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
