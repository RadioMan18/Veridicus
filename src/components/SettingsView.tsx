interface SettingsViewProps {
  workspacePath: string;
  gitActive: boolean;
  setupComplete: boolean;
  onOpenModels: () => void;
  onOpenSetup: () => void;
}

export default function SettingsView({
  workspacePath,
  gitActive,
  setupComplete,
  onOpenModels,
  onOpenSetup,
}: SettingsViewProps) {
  return (
    <div className="max-w-3xl space-y-4">
      <div>
        <h2 className="text-xl font-bold text-aether-text">Settings</h2>
        <p className="text-sm text-aether-text-muted">
          Local models, workspace and first-run setup.
        </p>
      </div>

      <section className="divide-y divide-aether-border rounded-xl border border-aether-border bg-aether-surface">
        <div className="flex flex-wrap items-center justify-between gap-3 p-4">
          <div>
            <p className="text-sm text-aether-text">Local models</p>
            <p className="text-xs text-aether-text-muted">
              Choose the fast and deep Ollama models used for generation.
            </p>
          </div>
          <button
            onClick={onOpenModels}
            className="rounded border border-aether-border px-3 py-1.5 text-sm text-aether-text transition hover:bg-aether-bg"
          >
            Configure
          </button>
        </div>

        <div className="flex flex-wrap items-center justify-between gap-3 p-4">
          <div>
            <p className="text-sm text-aether-text">Setup guide</p>
            <p className="text-xs text-aether-text-muted">
              {setupComplete
                ? 'Completed. Reopen to review your profile and claims.'
                : 'Not finished yet — import your resume and confirm claims.'}
            </p>
          </div>
          <button
            onClick={onOpenSetup}
            className="rounded border border-aether-border px-3 py-1.5 text-sm text-aether-text transition hover:bg-aether-bg"
          >
            Open wizard
          </button>
        </div>

        <div className="p-4">
          <p className="text-sm text-aether-text">Workspace</p>
          <p className="mt-1 break-all font-mono text-xs text-aether-text-muted">
            {workspacePath || 'Loading…'}
          </p>
          <p className="mt-1 text-xs text-aether-text-muted">
            {gitActive
              ? 'Local git history is active for your applications.'
              : 'No local git history yet.'}
          </p>
        </div>
      </section>

      <p className="text-xs text-aether-text-muted">
        CareerOS never sends your resume, claims or applications to a remote service. Generation
        runs against Ollama on this machine.
      </p>
    </div>
  );
}
