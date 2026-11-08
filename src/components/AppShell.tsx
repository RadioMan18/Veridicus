import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Briefcase,
  Compass,
  FileText,
  LayoutDashboard,
  Plus,
  Search,
  Settings as SettingsIcon,
  Target,
} from 'lucide-react';
import CommandPalette, { type PaletteEntry } from './CommandPalette';
import ApplicationLedger from './ApplicationLedger';
import ApplicationDetail from './ApplicationDetail';
import ExportResume from './ExportResume';
import OllamaSettings from './OllamaSettings';
import BriefingViewer from './BriefingViewer';
import SetupWizard from './SetupWizard';
import DashboardView from './DashboardView';
import DocumentsView from './DocumentsView';
import PrepView from './PrepView';
import FitView from './FitView';
import SettingsView from './SettingsView';
import Dialog from './Dialog';
import ShortcutSheet from './ShortcutSheet';
import Toaster from './Toaster';
import TrustStatus from './TrustStatus';

interface SetupWizardState {
  completed: boolean;
}

interface WorkspaceStatus {
  root_path: string;
  initialized: boolean;
  git_active: boolean;
}

interface AiDisclosureState {
  acknowledged: boolean;
  version: string;
  text: string;
}

type View = 'dashboard' | 'applications' | 'documents' | 'prep' | 'fit' | 'settings';

const NAV: { id: View; label: string; icon: typeof LayoutDashboard }[] = [
  { id: 'dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { id: 'applications', label: 'Applications', icon: Briefcase },
  { id: 'documents', label: 'Documents', icon: FileText },
  { id: 'prep', label: 'Prep', icon: Compass },
  { id: 'fit', label: 'Role fit', icon: Target },
  { id: 'settings', label: 'Settings', icon: SettingsIcon },
];

const CHORDS: Record<string, View> = {
  a: 'applications',
  d: 'dashboard',
  p: 'prep',
  s: 'settings',
};

function isTypingTarget(target: EventTarget | null) {
  const element = target as HTMLElement | null;
  return Boolean(element && ['INPUT', 'TEXTAREA', 'SELECT'].includes(element.tagName));
}

export default function AppShell() {
  const [view, setView] = useState<View>('dashboard');
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [initialCommand, setInitialCommand] = useState('');
  const [resumeStatus, setResumeStatus] = useState('');
  const [exportOpen, setExportOpen] = useState(false);
  const [exportFormat, setExportFormat] = useState<'pdf' | 'docx' | 'odt'>('pdf');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [setupOpen, setSetupOpen] = useState(false);
  const [setupComplete, setSetupComplete] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [briefingOpen, setBriefingOpen] = useState(false);
  const [briefingContent, setBriefingContent] = useState('');
  const [briefingTitle, setBriefingTitle] = useState('');
  const [applyResult, setApplyResult] = useState<any | null>(null);
  const [paletteHistory, setPaletteHistory] = useState<PaletteEntry[]>([]);
  const [workspace, setWorkspace] = useState<WorkspaceStatus | null>(null);
  const [disclosure, setDisclosure] = useState<AiDisclosureState | null>(null);
  const [acknowledgingDisclosure, setAcknowledgingDisclosure] = useState(false);
  const [isInitializing, setIsInitializing] = useState(true);
  const [initializationFailed, setInitializationFailed] = useState(false);
  const initializationPromise = useRef<Promise<void> | null>(null);
  const chordPending = useRef(false);

  const openCommandPalette = (cmd: string = '') => {
    setInitialCommand(cmd);
    setCommandPaletteOpen(true);
  };

  const closeCommandPalette = () => {
    setCommandPaletteOpen(false);
    setInitialCommand('');
  };

  useEffect(() => {
    if (!initializationPromise.current) {
      initializationPromise.current = (async () => {
        try {
          await invoke('initialize_workspace');
        } catch (e) {
          try {
            await invoke('get_workspace_status');
          } catch (e2) {
            console.error(e2);
          }
        }
        try {
          await invoke('init_encrypted_database_cmd');
        } catch (e) {
          console.error('DB init error:', e);
          throw e;
        }
        const wizardState = await invoke<SetupWizardState>('get_setup_wizard_state');
        setSetupComplete(wizardState.completed);
        if (!wizardState.completed && localStorage.getItem('careeros-setup-dismissed') !== 'true') {
          setSetupOpen(true);
        }
        try {
          const status = await invoke<WorkspaceStatus>('get_workspace_status');
          setWorkspace(status);
        } catch {
          // Workspace path and git state are informational only.
        }
      })();
    }

    let active = true;
    initializationPromise.current.then(
      () => {
        if (active) setIsInitializing(false);
      },
      (err) => {
        if (active) {
          setResumeStatus(`Initialization error: ${err.message || String(err)}`);
          setInitializationFailed(true);
          setIsInitializing(false);
        }
      },
    );

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (isInitializing || initializationFailed) return;
    let active = true;
    invoke<AiDisclosureState>('get_ai_disclosure_state')
      .then((state) => {
        if (active) setDisclosure(state);
      })
      .catch(() => {
        // The disclosure gate is retried on the next launch; do not block the app.
      });
    return () => {
      active = false;
    };
  }, [isInitializing, initializationFailed]);

  const acknowledgeAiDisclosure = async () => {
    setAcknowledgingDisclosure(true);
    try {
      await invoke('acknowledge_ai_disclosure');
      setDisclosure((current) => (current ? { ...current, acknowledged: true } : current));
    } catch (error: any) {
      setResumeStatus(`Could not record the disclosure acknowledgement: ${error?.message || String(error)}`);
    } finally {
      setAcknowledgingDisclosure(false);
    }
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        setCommandPaletteOpen((previous) => !previous);
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  // g-chords and the shortcut sheet. Dialogs own their own Escape/Tab behaviour.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (isTypingTarget(event.target)) return;

      if (event.key === '?') {
        event.preventDefault();
        setShortcutsOpen(true);
        return;
      }

      if (chordPending.current) {
        chordPending.current = false;
        const next = CHORDS[event.key.toLowerCase()];
        if (next) {
          event.preventDefault();
          setView(next);
        }
        return;
      }

      if (event.key === 'g') {
        chordPending.current = true;
        window.setTimeout(() => {
          chordPending.current = false;
        }, 1200);
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  const openResumeFile = async () => {
    try {
      await initializationPromise.current;
      const result: any = await invoke('open_resume_file', { filePath: null });
      setResumeStatus(
        result.success
          ? `✓ Resume imported. ${result.claims_imported} claims need confirmation in Setup before they can be used.`
          : `✗ Error: ${result.error_message}`,
      );
      if (result.success && result.claims_imported > 0) {
        setSetupOpen(true);
      }
    } catch (err: any) {
      setResumeStatus(`Error: ${err.message || String(err)}`);
    }
  };

  const openExport = (format: 'pdf' | 'docx' | 'odt' = 'pdf') => {
    setExportFormat(format);
    setExportOpen(true);
  };

  const showBriefing = (content: string, title: string) => {
    setBriefingContent(content);
    setBriefingTitle(title);
    setBriefingOpen(true);
  };

  const closeSetupWizard = () => {
    setSetupOpen(false);
    if (!setupComplete) {
      localStorage.setItem('careeros-setup-dismissed', 'true');
    }
  };

  const completeSetupWizard = () => {
    setSetupComplete(true);
    setSetupOpen(false);
  };

  const statusTone = initializationFailed
    ? 'bg-aether-error'
    : isInitializing
      ? 'bg-aether-warning'
      : 'bg-aether-success';
  const statusLabel = initializationFailed
    ? 'Initialization failed'
    : isInitializing
      ? 'Initializing'
      : 'Workspace ready';

  return (
    <div className="flex h-screen overflow-hidden bg-aether-bg text-aether-text">
      <aside className="flex w-60 shrink-0 flex-col border-r border-aether-border bg-aether-surface">
        <div className="px-5 py-5">
          <p className="text-row font-bold leading-tight">CareerOS</p>
          <p className="text-meta text-aether-text-muted">Local job search OS</p>
        </div>

        <nav className="flex-1 space-y-1 px-3">
          {NAV.map((item) => {
            const Icon = item.icon;
            const active = view === item.id;
            return (
              <button
                key={item.id}
                onClick={() => setView(item.id)}
                aria-current={active ? 'page' : undefined}
                className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-body transition ${
                  active
                    ? 'bg-aether-primary/15 font-medium text-aether-text'
                    : 'text-aether-text-muted hover:bg-aether-bg hover:text-aether-text'
                }`}
              >
                <Icon size={16} />
                {item.label}
              </button>
            );
          })}
        </nav>

        <div className="space-y-1 border-t border-aether-border p-3">
          <p className="flex items-center gap-2 text-meta text-aether-text-muted">
            <span className={`h-2 w-2 rounded-full ${statusTone}`} />
            {statusLabel}
          </p>
          <button
            onClick={() => setShortcutsOpen(true)}
            className="text-meta text-aether-text-muted transition hover:text-aether-text"
          >
            Keyboard shortcuts (?)
          </button>
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center gap-3 border-b border-aether-border bg-aether-surface px-6 py-3">
          <button
            onClick={() => openCommandPalette('')}
            className="flex min-w-0 flex-1 items-center gap-2 rounded-lg border border-aether-border bg-aether-bg px-3 py-2 text-body text-aether-text-muted transition hover:border-aether-primary"
          >
            <Search size={14} />
            <span className="truncate">Search or run a command</span>
            <span className="ml-auto shrink-0 rounded border border-aether-border px-1.5 py-0.5 font-mono text-meta">
              Ctrl K
            </span>
          </button>
          <button
            onClick={() => openCommandPalette('/apply ')}
            className="flex shrink-0 items-center gap-2 rounded-lg bg-aether-primary px-4 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover"
          >
            <Plus size={16} /> New application
          </button>
          <TrustStatus />
        </header>

        <main className="flex-1 overflow-y-auto p-6">
          {view === 'dashboard' && (
            <DashboardView
              resumeStatus={resumeStatus}
              onStartApply={() => openCommandPalette('/apply ')}
              onOpenApplications={() => setView('applications')}
              onOpenPrep={() => setView('prep')}
              onOpenDocuments={() => setView('documents')}
            />
          )}
          {view === 'applications' && (
            <ApplicationLedger onStartApply={() => openCommandPalette('/apply ')} />
          )}
          {view === 'documents' && (
            <DocumentsView
              workspacePath={workspace?.root_path || ''}
              gitActive={Boolean(workspace?.git_active)}
              resumeStatus={resumeStatus}
              busy={isInitializing || initializationFailed}
              onImportResume={openResumeFile}
              onExport={openExport}
            />
          )}
          {view === 'prep' && <PrepView onOpenFit={() => setView('fit')} />}
          {view === 'fit' && <FitView onStartApply={() => openCommandPalette('/apply ')} />}
          {view === 'settings' && (
            <SettingsView
              workspacePath={workspace?.root_path || ''}
              gitActive={Boolean(workspace?.git_active)}
              setupComplete={setupComplete}
              onOpenModels={() => setSettingsOpen(true)}
              onOpenSetup={() => setSetupOpen(true)}
            />
          )}
        </main>
      </div>

      <CommandPalette
        isOpen={commandPaletteOpen}
        onClose={closeCommandPalette}
        onExport={openExport}
        onShowBriefing={showBriefing}
        initialCommand={initialCommand}
        history={paletteHistory}
        onHistory={(entry) => setPaletteHistory((previous) => [...previous, entry].slice(-60))}
        onApplyComplete={(result) => setApplyResult(result)}
        onOpenFit={() => setView('fit')}
      />
      {applyResult && (
        <ApplicationDetail result={applyResult} onClose={() => setApplyResult(null)} />
      )}
      <ExportResume
        isOpen={exportOpen}
        initialFormat={exportFormat}
        onClose={() => setExportOpen(false)}
      />
      <OllamaSettings isOpen={settingsOpen} onClose={() => setSettingsOpen(false)} />
      <SetupWizard isOpen={setupOpen} onClose={closeSetupWizard} onComplete={completeSetupWizard} />
      <BriefingViewer
        isOpen={briefingOpen}
        title={briefingTitle}
        markdown={briefingContent}
        onClose={() => setBriefingOpen(false)}
      />
      <ShortcutSheet isOpen={shortcutsOpen} onClose={() => setShortcutsOpen(false)} />
      <Dialog
        isOpen={Boolean(disclosure && !disclosure.acknowledged)}
        onClose={() => {}}
        title="Before you begin"
        description="AI-assisted drafting and your responsibility"
        size="md"
        footer={
          <button
            onClick={acknowledgeAiDisclosure}
            disabled={acknowledgingDisclosure}
            data-autofocus
            className="rounded bg-aether-primary px-4 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
          >
            {acknowledgingDisclosure ? 'Recording…' : 'I understand'}
          </button>
        }
      >
        <div className="space-y-3 text-sm leading-6 text-aether-text-muted">
          <p>{disclosure?.text}</p>
          <ul className="list-disc space-y-1 pl-5">
            <li>Every generated document is a draft, not a verified fact.</li>
            <li>Claims are checked for consistency, not for whether they are true.</li>
            <li>Your acknowledgement is recorded in the workspace compliance record.</li>
          </ul>
        </div>
      </Dialog>
      <Toaster />
    </div>
  );
}
