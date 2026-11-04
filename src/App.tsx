import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import CommandPalette from './components/CommandPalette';
import ApplicationLedger from './components/ApplicationLedger';
import ExportResume from './components/ExportResume';
import OllamaSettings from './components/OllamaSettings';
import BriefingViewer from './components/BriefingViewer';
import SetupWizard from './components/SetupWizard';

interface SetupWizardState {
  completed: boolean;
}

function App() {
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [initialCommand, setInitialCommand] = useState('');
  const [activeView, setActiveView] = useState<'dashboard' | 'ledger'>('dashboard');
  const [resumeStatus, setResumeStatus] = useState<string>('');
  const [exportOpen, setExportOpen] = useState(false);
  const [exportFormat, setExportFormat] = useState<'pdf' | 'docx' | 'odt'>('pdf');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [setupOpen, setSetupOpen] = useState(false);
  const [setupComplete, setSetupComplete] = useState(false);
  const [briefingOpen, setBriefingOpen] = useState(false);
  const [briefingContent, setBriefingContent] = useState('');
  const [briefingTitle, setBriefingTitle] = useState('');
  const [isInitializing, setIsInitializing] = useState(true);
  const [initializationFailed, setInitializationFailed] = useState(false);
  const initializationPromise = useRef<Promise<void> | null>(null);

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
            // If workspace exists, try to init db anyway
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
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        setCommandPaletteOpen(!commandPaletteOpen);
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [commandPaletteOpen]);

  const openResumeFile = async () => {
    try {
      await initializationPromise.current;
      const result: any = await invoke('open_resume_file', { filePath: null });
      setResumeStatus(result.success
        ? `✓ Resume imported! ${result.claims_imported} claims saved to ledger.`
        : `✗ Error: ${result.error_message}`);
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

  return (
    <div className="min-h-screen bg-aether-bg text-aether-text p-6 transition-colors duration-300">
      <header className="mb-8 flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
        <div>
          <h1 className="text-2xl font-bold text-aether-text">CareerOS</h1>
          <p className="text-aether-text-muted text-sm">Privacy-first job search OS with honesty guardrails</p>
        </div>
        <div className="flex flex-wrap gap-3">
          <button
            onClick={() => setActiveView('dashboard')}
            className={`px-4 py-2 rounded bg-aether-surface border border-aether-border hover:bg-aether-primary/10 text-aether-text transition ${activeView === 'dashboard' ? 'border-b-2 border-aether-primary' : ''}`}
          >
            Dashboard
          </button>
          <button
            onClick={() => setActiveView('ledger')}
            className={`px-4 py-2 rounded bg-aether-surface border border-aether-border hover:bg-aether-primary/10 text-aether-text transition ${activeView === 'ledger' ? 'border-b-2 border-aether-primary' : ''}`}
          >
            Application Ledger
          </button>
          <button
            onClick={openResumeFile}
            disabled={isInitializing || initializationFailed}
            className="px-4 py-2 rounded bg-aether-selected text-aether-selected-fg hover:bg-aether-selected-border font-medium transition border border-aether-selected-border disabled:opacity-50 disabled:cursor-wait"
          >
            {isInitializing ? 'Initializing...' : initializationFailed ? 'Resume Import Unavailable' : 'Open Resume File'}
          </button>
          <button
            onClick={() => openExport()}
            className="px-4 py-2 rounded bg-aether-warning text-aether-primary-fg hover:bg-aether-warning-hover font-medium transition border border-aether-warning"
          >
            Export Resume
          </button>
          <button
            onClick={() => setSetupOpen(true)}
            disabled={isInitializing || initializationFailed}
            className="px-4 py-2 rounded bg-aether-surface border border-aether-border hover:bg-aether-primary/10 text-aether-text transition disabled:opacity-50 disabled:cursor-wait"
          >
            Setup guide
          </button>
          <button
            onClick={() => setSettingsOpen(true)}
            className="px-4 py-2 rounded bg-aether-surface border border-aether-border hover:bg-aether-primary/10 text-aether-text transition"
          >
            Ollama Settings
          </button>
          <button
            onClick={() => openCommandPalette('')}
            className="px-4 py-2 rounded bg-aether-primary text-aether-primary-fg hover:bg-aether-primary-hover font-medium transition"
          >
            Cmd+K
          </button>
        </div>
      </header>

      <main className="space-y-6">
        {activeView === 'dashboard' && (
          <section className="bg-aether-surface border border-aether-border rounded-xl p-6">
            <h2 className="text-xl font-bold text-aether-text mb-4">Quick Start</h2>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <div className="p-4 bg-aether-bg rounded border border-aether-border">
                <h3 className="font-semibold text-aether-text mb-2">/apply &lt;url&gt;</h3>
                <p className="text-sm text-aether-text-muted mb-3">Scrape a job posting, run gap analysis, and generate tailored resume + cover letter.</p>
                <button onClick={() => openCommandPalette('/apply ')} className="px-3 py-1 bg-aether-primary text-aether-primary-fg hover:bg-aether-primary-hover text-xs rounded transition">Try /apply</button>
              </div>
              <div className="p-4 bg-aether-bg rounded border border-aether-border">
                <h3 className="font-semibold text-aether-text mb-2">/prep &lt;company&gt;</h3>
                <p className="text-sm text-aether-text-muted mb-3">Generate a company research briefing for interviews.</p>
                <button onClick={() => openCommandPalette('/prep ')} className="px-3 py-1 bg-aether-primary text-aether-primary-fg hover:bg-aether-primary-hover text-xs rounded transition">Try /prep</button>
              </div>
              <div className="p-4 bg-aether-bg rounded border border-aether-border">
                <h3 className="font-semibold text-aether-text mb-2">/mock &lt;role&gt;</h3>
                <p className="text-sm text-aether-text-muted mb-3">STAR-method interview simulator with hiring-manager character.</p>
                <button onClick={() => openCommandPalette('/mock ')} className="px-3 py-1 bg-aether-primary text-aether-primary-fg hover:bg-aether-primary-hover text-xs rounded transition">Try /mock</button>
              </div>
              <div className="p-4 bg-aether-bg rounded border border-aether-border">
                <h3 className="font-semibold text-aether-text mb-2">/resume &lt;filepath&gt;</h3>
                <p className="text-sm text-aether-text-muted mb-3">Import a resume file and extract claims to the ledger.</p>
                <button onClick={() => openCommandPalette('/resume ')} className="px-3 py-1 bg-aether-selected text-aether-selected-fg hover:bg-aether-selected-border text-xs rounded transition border border-aether-selected-border">Try /resume</button>
              </div>
              <div className="p-4 bg-aether-bg rounded border border-aether-border">
                <h3 className="font-semibold text-aether-text mb-2">/export &lt;format&gt;</h3>
                <p className="text-sm text-aether-text-muted mb-3">Export ATS-optimized resume as PDF, DOCX, or ODT.</p>
                <button onClick={() => openCommandPalette('/export ')} className="px-3 py-1 bg-aether-warning text-aether-primary-fg hover:bg-aether-warning-hover text-xs rounded transition border border-aether-warning">Try /export</button>
              </div>
            </div>
            {resumeStatus && (
              <div className="mt-4 p-3 rounded bg-aether-bg border border-aether-border text-sm">
                <p className={resumeStatus.startsWith('✓') ? 'text-aether-success' : 'text-aether-error'}>{resumeStatus}</p>
              </div>
            )}
          </section>
        )}

        {activeView === 'ledger' && <ApplicationLedger />}
      </main>

      <CommandPalette
        isOpen={commandPaletteOpen}
        onClose={closeCommandPalette}
        onExport={openExport}
        onShowBriefing={showBriefing}
        initialCommand={initialCommand}
      />
      <ExportResume
        isOpen={exportOpen}
        initialFormat={exportFormat}
        onClose={() => setExportOpen(false)}
      />
      <OllamaSettings isOpen={settingsOpen} onClose={() => setSettingsOpen(false)} />
      <SetupWizard
        isOpen={setupOpen}
        onClose={closeSetupWizard}
        onComplete={completeSetupWizard}
      />
      <BriefingViewer
        isOpen={briefingOpen}
        title={briefingTitle}
        markdown={briefingContent}
        onClose={() => setBriefingOpen(false)}
      />
    </div>
  );
}

export default App;
