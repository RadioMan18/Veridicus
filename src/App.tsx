import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import CommandPalette from './components/CommandPalette';
import ApplicationLedger from './components/ApplicationLedger';
import ExportResume from './components/ExportResume';

function App() {
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [activeView, setActiveView] = useState<'dashboard' | 'ledger'>('dashboard');
  const [resumeStatus, setResumeStatus] = useState<string>('');
  const [exportOpen, setExportOpen] = useState(false);

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
      const result: any = await invoke('open_resume_file', { filePath: null });
      setResumeStatus(result.success
        ? `✓ Resume imported! ${result.claims_imported} claims saved to ledger.`
        : `✗ Error: ${result.error_message}`);
    } catch (err: any) {
      setResumeStatus(`Error: ${err.message || String(err)}`);
    }
  };

  const openExport = () => setExportOpen(true);

  return (
    <div className="min-h-screen bg-aether-bg text-aether-text p-6 transition-colors duration-300">
      <header className="mb-8 flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-aether-text">CareerOS</h1>
          <p className="text-aether-text-muted text-sm">Privacy-first job search OS with honesty guardrails</p>
        </div>
        <div className="flex gap-3">
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
            className="px-4 py-2 rounded bg-aether-selected text-aether-selected-fg hover:bg-aether-selected-border font-medium transition border border-aether-selected-border"
          >
            Open Resume File
          </button>
          <button
            onClick={openExport}
            className="px-4 py-2 rounded bg-aether-warning text-aether-primary-fg hover:bg-aether-warning-hover font-medium transition border border-aether-warning"
          >
            Export Resume
          </button>
          <button
            onClick={() => setCommandPaletteOpen(true)}
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
                <button className="px-3 py-1 bg-aether-primary text-aether-primary-fg hover:bg-aether-primary-hover text-xs rounded transition">Try /apply</button>
              </div>
              <div className="p-4 bg-aether-bg rounded border border-aether-border">
                <h3 className="font-semibold text-aether-text mb-2">/prep &lt;company&gt;</h3>
                <p className="text-sm text-aether-text-muted mb-3">Generate a company research briefing for interviews.</p>
                <button className="px-3 py-1 bg-aether-primary text-aether-primary-fg hover:bg-aether-primary-hover text-xs rounded transition">Try /prep</button>
              </div>
              <div className="p-4 bg-aether-bg rounded border border-aether-border">
                <h3 className="font-semibold text-aether-text mb-2">/mock &lt;role&gt;</h3>
                <p className="text-sm text-aether-text-muted mb-3">STAR-method interview simulator with hiring-manager character.</p>
                <button className="px-3 py-1 bg-aether-primary text-aether-primary-fg hover:bg-aether-primary-hover text-xs rounded transition">Try /mock</button>
              </div>
              <div className="p-4 bg-aether-bg rounded border border-aether-border">
                <h3 className="font-semibold text-aether-text mb-2">/resume &lt;filepath&gt;</h3>
                <p className="text-sm text-aether-text-muted mb-3">Import a resume file and extract claims to the ledger.</p>
                <button className="px-3 py-1 bg-aether-selected text-aether-selected-fg hover:bg-aether-selected-border text-xs rounded transition border border-aether-selected-border">Try /resume</button>
              </div>
              <div className="p-4 bg-aether-bg rounded border border-aether-border">
                <h3 className="font-semibold text-aether-text mb-2">/export &lt;format&gt;</h3>
                <p className="text-sm text-aether-text-muted mb-3">Export ATS-optimized resume as PDF, DOCX, or ODT.</p>
                <button className="px-3 py-1 bg-aether-warning text-aether-primary-fg hover:bg-aether-warning-hover text-xs rounded transition border border-aether-warning">Try /export</button>
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
        onClose={() => setCommandPaletteOpen(false)}
      />
      <ExportResume onClose={() => setExportOpen(false)} />
    </div>
  );
}

export default App;
