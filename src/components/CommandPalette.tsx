import { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { CornerDownLeft } from 'lucide-react';
import ApplyTimeline, { initialStageStates, type StageState } from './ApplyTimeline';
import Markdown from './Markdown';

export interface PaletteEntry {
  kind: 'info' | 'success' | 'error';
  text: string;
}

interface PaletteCommand {
  id: string;
  description: string;
  hint: string;
  example: string;
  takesArgument: boolean;
}

const COMMANDS: PaletteCommand[] = [
  {
    id: '/apply',
    description: 'Scrape a posting and build a tailored resume and cover letter',
    hint: 'a job posting URL',
    example: '/apply https://boards.greenhouse.io/acme/jobs/123',
    takesArgument: true,
  },
  {
    id: '/prep',
    description: 'Company research briefing for an interview',
    hint: 'a company name',
    example: '/prep NetDocuments',
    takesArgument: true,
  },
  {
    id: '/mock',
    description: 'STAR-method interview simulator',
    hint: 'a role title',
    example: '/mock Director of Training',
    takesArgument: true,
  },
  {
    id: '/resume',
    description: 'Import a resume file and extract claims into the ledger',
    hint: 'an optional file path',
    example: '/resume C:\\Users\\me\\resume.docx',
    takesArgument: false,
  },
  {
    id: '/export',
    description: 'Export the master resume as PDF, DOCX or ODT',
    hint: 'pdf, docx or odt',
    example: '/export docx',
    takesArgument: true,
  },
  {
    id: '/followup',
    description: 'Draft follow-up emails for stale applications',
    hint: 'no arguments',
    example: '/followup',
    takesArgument: false,
  },
  {
    id: '/weekly',
    description: 'Weekly application counts by status',
    hint: 'no arguments',
    example: '/weekly',
    takesArgument: false,
  },
  {
    id: '/logs',
    description: 'Daily log and application activity analysis',
    hint: 'no arguments',
    example: '/logs',
    takesArgument: false,
  },
];

const RECENTS_KEY = 'careeros-recent-commands';

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  onExport: (format: 'pdf' | 'docx' | 'odt') => void;
  onShowBriefing?: (content: string, title: string) => void;
  initialCommand?: string;
  history: PaletteEntry[];
  onHistory: (entry: PaletteEntry) => void;
  onApplyComplete?: (result: any) => void;
}

export default function CommandPalette({
  isOpen,
  onClose,
  onExport,
  onShowBriefing,
  initialCommand = '',
  history,
  onHistory,
  onApplyComplete,
}: CommandPaletteProps) {
  const [command, setCommand] = useState(initialCommand);
  const [selected, setSelected] = useState(0);
  const [running, setRunning] = useState(false);
  const [applyStates, setApplyStates] = useState<Record<string, StageState>>(initialStageStates);
  const [applyModel, setApplyModel] = useState<string | undefined>(undefined);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [recents, setRecents] = useState<string[]>([]);
  const outputRef = useRef<HTMLDivElement | null>(null);
  const executingRef = useRef(false);
  const progressWarnedRef = useRef(false);

  useEffect(() => {
    if (!isOpen) return;
    setCommand(initialCommand);
    setSelected(0);
    try {
      const stored = JSON.parse(localStorage.getItem(RECENTS_KEY) || '[]');
      setRecents(Array.isArray(stored) ? stored : []);
    } catch {
      setRecents([]);
    }
  }, [isOpen, initialCommand]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;
    listen<{ stage: string; status: string; detail?: string | null }>('apply-progress', (event) => {
      const { stage, status, detail } = event.payload;
      setApplyStates((previous) => ({
        ...previous,
        [stage]: { status: status as StageState['status'], detail: detail || undefined },
      }));
    }).then((stop) => {
      if (active) unlisten = stop;
      else stop();
    }).catch(() => {
      // Without the core:event permission the timeline cannot update; say so instead
      // of showing an empty checklist that never advances.
      if (active && !progressWarnedRef.current) {
        progressWarnedRef.current = true;
        onHistory({
          kind: 'error',
          text: 'Live progress is unavailable in this build. The run still finishes; results appear when it completes.',
        });
      }
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    const onEscape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      if (running) {
        onHistory({ kind: 'info', text: 'A run is in progress. Use Cancel run to stop it.' });
        return;
      }
      onClose();
    };
    window.addEventListener('keydown', onEscape);
    return () => window.removeEventListener('keydown', onEscape);
  }, [isOpen, running, onClose, onHistory]);

  useEffect(() => {
    if (!running) return;
    const startedAt = Date.now() - elapsedMs;
    const timer = setInterval(() => setElapsedMs(Date.now() - startedAt), 250);
    return () => clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [running]);

  useEffect(() => {
    if (outputRef.current) outputRef.current.scrollTop = outputRef.current.scrollHeight;
  }, [history.length, applyStates]);

  const ordered = useMemo(() => {
    return [...COMMANDS].sort((a, b) => {
      const aIndex = recents.indexOf(a.id);
      const bIndex = recents.indexOf(b.id);
      if (aIndex === -1 && bIndex === -1) return 0;
      if (aIndex === -1) return 1;
      if (bIndex === -1) return -1;
      return aIndex - bIndex;
    });
  }, [recents]);

  const typed = command.trim();
  const commandWord = (typed.startsWith('/') ? typed.split(/\s+/)[0] : typed).toLowerCase();
  const argument = typed.startsWith('/') ? typed.slice(commandWord.length).trim() : '';
  const showList = !typed.includes(' ');

  const matches = useMemo(() => {
    if (!commandWord) return ordered;
    const needle = commandWord.replace(/^\//, '');
    return ordered.filter(
      (item) =>
        item.id.slice(1).startsWith(needle) || item.description.toLowerCase().includes(needle),
    );
  }, [ordered, commandWord]);

  const selectedCommand = matches[Math.min(selected, Math.max(matches.length - 1, 0))];

  const remember = (id: string) => {
    const next = [id, ...recents.filter((entry) => entry !== id)].slice(0, 4);
    setRecents(next);
    try {
      localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
    } catch {
      // Recents are a convenience; ignore storage failures.
    }
  };

  const runApply = async (url: string) => {
    setApplyStates(initialStageStates());
    setApplyModel(undefined);
    setElapsedMs(0);
    setRunning(true);
    onHistory({ kind: 'info', text: `Applying: ${url}` });

    try {
      const config: any = await invoke('get_ollama_config').catch(() => null);
      if (config?.deep_model || config?.fast_model) {
        setApplyModel(config.deep_model || config.fast_model);
      }
    } catch {
      // The model name is cosmetic.
    }

    try {
      const result: any = await invoke('execute_apply_workflow', {
        input: {
          jd_url: url,
          employer: null,
          role: null,
          req_id: null,
          defer_commit: true,
        },
      });
      if (result.success) {
        onHistory({
          kind: 'success',
          text: `Materials ready: ${result.employer} — ${result.role}. Nothing committed yet.`,
        });
        onApplyComplete?.(result);
        onClose();
      } else {
        onHistory({
          kind: 'error',
          text: `Apply stopped: ${result.error_message || 'unknown error'}`,
        });
      }
    } catch (error: any) {
      onHistory({ kind: 'error', text: `Apply failed: ${error?.message || String(error)}` });
    } finally {
      setRunning(false);
    }
  };

  const execute = async () => {
    if (executingRef.current || running) return;

    const known = COMMANDS.find((item) => item.id === commandWord);
    if (!known) {
      const needle = commandWord.replace(/^\//, '').slice(0, 2);
      const guess = COMMANDS.find((item) => item.id.slice(1).startsWith(needle));
      onHistory({
        kind: 'error',
        text: guess
          ? `Unknown command. Did you mean ${guess.id}? Type / to see all eight.`
          : 'Unknown command. Type / to see all eight commands.',
      });
      return;
    }

    if (known.takesArgument && !argument) {
      onHistory({
        kind: 'error',
        text: `${known.id} needs ${known.hint}. Example: ${known.example}`,
      });
      return;
    }

    remember(known.id);
    executingRef.current = true;

    try {
      if (known.id === '/apply') {
        if (!/^https?:\/\//i.test(argument)) {
          onHistory({
            kind: 'error',
            text: 'That is not a URL. Paste the full https:// address of the job posting.',
          });
          return;
        }
        await runApply(argument);
        return;
      }

      setRunning(true);

      if (known.id === '/prep') {
        const briefing: string = await invoke('run_company_research', { company: argument });
        onShowBriefing?.(briefing, argument);
        onHistory({ kind: 'success', text: `Briefing ready for ${argument}.` });
        onClose();
        return;
      }

      if (known.id === '/mock') {
        const intro: string = await invoke('start_interview_simulator', { role: argument });
        onHistory({ kind: 'success', text: intro });
        return;
      }

      if (known.id === '/followup') {
        const drafts: string = await invoke('draft_followups');
        onHistory({ kind: 'success', text: drafts });
        return;
      }

      if (known.id === '/weekly') {
        const report: string = await invoke('generate_weekly_report');
        onHistory({ kind: 'success', text: report });
        return;
      }

      if (known.id === '/logs') {
        const analysis: string = await invoke('analyze_logs');
        onHistory({ kind: 'success', text: analysis });
        return;
      }

      if (known.id === '/resume') {
        const result: any = await invoke('open_resume_file', { filePath: argument || null });
        onHistory(
          result.success
            ? { kind: 'success', text: `Resume imported. ${result.claims_imported} claims saved.` }
            : { kind: 'error', text: result.error_message || 'Resume import failed.' },
        );
        return;
      }

      if (known.id === '/export') {
        const format = argument.toLowerCase();
        if (format !== 'pdf' && format !== 'docx' && format !== 'odt') {
          onHistory({ kind: 'error', text: 'Use /export pdf, /export docx or /export odt.' });
          return;
        }
        onExport(format);
        onClose();
      }
    } catch (error: any) {
      onHistory({ kind: 'error', text: error?.message || String(error) });
    } finally {
      executingRef.current = false;
      setRunning(false);
    }
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      if (running) {
        onHistory({ kind: 'info', text: 'A run is in progress. Use Cancel run to stop it.' });
        return;
      }
      onClose();
      return;
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setSelected((index) => Math.min(index + 1, Math.max(matches.length - 1, 0)));
      return;
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault();
      setSelected((index) => Math.max(index - 1, 0));
      return;
    }

    if (event.key === 'Tab') {
      event.preventDefault();
      if (selectedCommand) {
        setCommand(`${selectedCommand.id} `);
        setSelected(0);
      }
      return;
    }

    if (event.key === 'Enter') {
      event.preventDefault();
      if (showList && selectedCommand && selectedCommand.takesArgument && !argument) {
        setCommand(`${selectedCommand.id} `);
        setSelected(0);
        return;
      }
      void execute();
    }
  };

  if (!isOpen) return null;

  return (
    <div
      className="dialog-backdrop fixed inset-0 z-50 flex items-start justify-center bg-black/60 p-4 pt-[10vh] backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
    >
      <div className="flex max-h-[80vh] w-full max-w-3xl flex-col overflow-hidden rounded-xl border border-aether-border bg-aether-surface shadow-2xl">
        <div className="flex items-center gap-2 border-b border-aether-border p-4">
          <span className="font-mono font-bold text-aether-primary-ink">&gt;_</span>
          <input
            type="text"
            value={command}
            onChange={(event) => {
              setCommand(event.target.value);
              setSelected(0);
            }}
            onKeyDown={handleKeyDown}
            placeholder="Type / to see every command, e.g. /apply https://company.com/job"
            className="w-full bg-transparent font-mono text-aether-text outline-none placeholder:text-aether-text-muted/70"
            autoFocus
            aria-label="Command input"
          />
        </div>

        {showList && matches.length > 0 && (
          <ul className="max-h-64 overflow-y-auto border-b border-aether-border" role="listbox">
            {matches.map((item, index) => (
              <li key={item.id}>
                <button
                  onMouseEnter={() => setSelected(index)}
                  onClick={() => {
                    setCommand(`${item.id} `);
                    setSelected(0);
                  }}
                  className={`flex w-full items-start gap-3 px-4 py-2 text-left transition ${
                    index === selected ? 'bg-aether-primary/15' : 'hover:bg-aether-primary/10'
                  }`}
                >
                  <span className="w-24 shrink-0 font-mono text-meta text-aether-primary-ink">
                    {item.id}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block text-sm text-aether-text">{item.description}</span>
                    <span className="block truncate text-xs text-aether-text-muted">
                      {item.takesArgument ? `takes ${item.hint}` : item.hint} · {item.example}
                    </span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}

        {showList && matches.length === 0 && (
          <div className="border-b border-aether-border px-4 py-3 text-sm text-aether-text-muted">
            No command matches “{commandWord}”. Type / to list all eight.
          </div>
        )}

        {!showList && (
          <div className="border-b border-aether-border px-4 py-2 font-mono text-xs text-aether-text-muted">
            {selectedCommand?.id === commandWord || !selectedCommand
              ? COMMANDS.find((item) => item.id === commandWord)?.hint
              : `tab completes ${selectedCommand?.id}`}
            {' · '}
            enter runs
          </div>
        )}

        <div
          ref={outputRef}
          className="flex-1 overflow-y-auto whitespace-pre-wrap bg-aether-bg p-4 text-sm text-aether-text"
        >
          {running && (
            <div className="mb-3">
              <ApplyTimeline
                states={applyStates}
                elapsedMs={elapsedMs}
                model={applyModel}
                running={running}
                onCancel={() => {
                  void invoke('cancel_apply');
                  onHistory({ kind: 'info', text: 'Cancellation requested…' });
                }}
              />
            </div>
          )}

          {history.length === 0 && !running && (
            <p className="text-aether-text-muted">
              Ready. Type / to browse commands, Tab to complete, Enter to run.
            </p>
          )}

          {history.map((entry, index) =>
            entry.kind === 'error' ? (
              <p
                key={`${index}-error`}
                className="mt-2 whitespace-pre-wrap font-mono text-xs text-aether-error"
              >
                {entry.text}
              </p>
            ) : (
              <div key={`${index}-${entry.kind}`} className="mt-2">
                <Markdown content={entry.text} />
              </div>
            ),
          )}
        </div>

        <div className="flex flex-wrap justify-between gap-2 border-t border-aether-border bg-aether-surface p-2 text-xs text-aether-text-muted">
          <span className="flex items-center gap-1">
            <CornerDownLeft size={12} /> run · tab complete · ↑↓ browse
          </span>
          <span>Esc closes · Ctrl+K toggles</span>
        </div>
      </div>
    </div>
  );
}
