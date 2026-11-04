import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  onExport: (format: 'pdf' | 'docx' | 'odt') => void;
  onShowBriefing?: (content: string, title: string) => void;
  initialCommand?: string;
}

export default function CommandPalette({ isOpen, onClose, onExport, onShowBriefing, initialCommand = '' }: CommandPaletteProps) {
  const [command, setCommand] = useState(initialCommand);
  const [status, setStatus] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const executingRef = useRef(false);

  useEffect(() => {
    if (isOpen) {
      setCommand(initialCommand);
      setStatus('');
    } else {
      setCommand('');
      setStatus('');
    }
  }, [isOpen, initialCommand]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  const executeCommand = async () => {
    if (!command.trim() || executingRef.current) return;
    executingRef.current = true;
    setLoading(true);
    setStatus('Executing workflow...');

    try {
      const cmd = command.trim();

      if (cmd.startsWith('/apply ')) {
        const url = cmd.replace('/apply ', '').trim();
        const result: any = await invoke('execute_apply_workflow', {
          input: {
            jd_url: url,
            employer: null,
            role: null,
            req_id: null,
          },
        });
        setStatus(result.success
          ? `✓ Application created! ID: ${result.application_id}. PDF: ${result.pdf_compiled?.success ? 'compiled' : 'failed'}. Git: ${result.git_commit ? 'committed' : 'failed'}.`
          : `✗ Error: ${result.error_message}`);
      } else if (cmd.startsWith('/prep ')) {
        const company = cmd.replace('/prep ', '').trim();
        const briefing: string = await invoke('run_company_research', { company });
        if (onShowBriefing) {
          onShowBriefing(briefing, company);
          onClose();
        } else {
          setStatus(briefing);
        }
      } else if (cmd.startsWith('/mock ')) {
        const role = cmd.replace('/mock ', '').trim();
        const intro: string = await invoke('start_interview_simulator', { role });
        setStatus(intro);
      } else if (cmd.startsWith('/followup')) {
        const drafts: string = await invoke('draft_followups');
        setStatus(drafts);
      } else if (cmd.startsWith('/weekly')) {
        const report: string = await invoke('generate_weekly_report');
        setStatus(report);
      } else if (cmd.startsWith('/logs')) {
        const analysis: string = await invoke('analyze_logs');
        setStatus(analysis);
      } else if (cmd.startsWith('/resume')) {
        const arg = cmd.replace('/resume', '').trim();
        const result: any = await invoke('open_resume_file', {
          file_path: arg || null,
        });
        setStatus(result.success
          ? `✓ Resume imported! ${result.claims_imported} claims saved to ledger.`
          : `✗ Error: ${result.error_message}`);
      } else if (cmd === '/export' || cmd.startsWith('/export ')) {
        const format = cmd.slice('/export'.length).trim().toLowerCase();
        switch (format) {
          case '':
          case 'pdf':
            onExport('pdf');
            break;
          case 'docx':
            onExport('docx');
            break;
          case 'odt':
            onExport('odt');
            break;
          default:
            setStatus('Unsupported export format. Use /export PDF, /export DOCX, or /export ODT.');
            return;
        }
        onClose();
      } else {
        setStatus('Unknown command. Available: /apply <url>, /prep <company>, /mock <role>, /resume [filepath], /export [PDF|DOCX|ODT], /followup, /weekly, /logs');
      }
    } catch (err: any) {
      setStatus(`Error: ${err.message || String(err)}`);
    } finally {
      executingRef.current = false;
      setLoading(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 z-50">
      <div className="bg-aether-surface border border-aether-border rounded-xl w-full max-w-2xl overflow-hidden shadow-2xl">
        <div className="p-4 border-b border-aether-border flex items-center gap-2">
          <span className="text-aether-primary font-mono font-bold">&gt;_</span>
          <input
            type="text"
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && executeCommand()}
            placeholder="Type a slash command, e.g. /apply https://company.com/job..."
            className="w-full bg-transparent text-aether-text outline-none font-mono"
            autoFocus
          />
        </div>
        <div className="p-4 bg-aether-bg text-sm font-mono min-h-[150px] overflow-y-auto text-aether-text">
          {loading ? <p className="animate-pulse text-aether-primary">Processing with local model...</p> : <p className="text-aether-text">{status || 'Ready for command input.'}</p>}
        </div>
        <div className="p-2 bg-aether-surface border-t border-aether-border text-xs text-aether-text-muted flex justify-between">
          <span>Press <strong className="text-aether-text">Enter</strong> to run • <strong className="text-aether-text">Esc</strong> to exit</span>
          <span>Cmd+K / Ctrl+K to toggle</span>
        </div>
      </div>
    </div>
  );
}