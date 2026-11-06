import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import Dialog from './Dialog';
import { pushToast } from './Toaster';

interface OllamaSettingsProps {
  isOpen: boolean;
  onClose: () => void;
}

interface OllamaModel {
  name: string;
  size: number;
  modified_at: string;
}

export default function OllamaSettings({ isOpen, onClose }: OllamaSettingsProps) {
  const [models, setModels] = useState<OllamaModel[]>([]);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState('');
  const [statusKind, setStatusKind] = useState<'ok' | 'error' | 'info'>('info');
  const [fast, setFast] = useState('');
  const [deep, setDeep] = useState('');
  const [newModel, setNewModel] = useState('');

  const loadModels = async () => {
    setLoading(true);
    setStatus('Loading models…');
    setStatusKind('info');
    try {
      const result: OllamaModel[] = await invoke('get_ollama_models');
      setModels(result);
      setStatus(result.length > 0 ? `${result.length} models installed` : 'No models found');
      setStatusKind('ok');

      try {
        const config: any = await invoke('get_ollama_config');
        if (config.fast_model) setFast(config.fast_model);
        if (config.deep_model) setDeep(config.deep_model);
      } catch {
        // Model roles are optional until the user picks them.
      }
    } catch (err: any) {
      setStatus(`Error: ${err.message || String(err)}`);
      setStatusKind('error');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (isOpen) void loadModels();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen]);

  const saveConfig = async () => {
    setLoading(true);
    setStatus('Saving configuration…');
    setStatusKind('info');
    try {
      await invoke('save_ollama_config', { fastModel: fast || null, deepModel: deep || null });
      setStatus('Configuration saved');
      setStatusKind('ok');
      pushToast({ kind: 'success', message: 'Model roles saved' });
    } catch (err: any) {
      const message = err.message || String(err);
      setStatus(`Error: ${message}`);
      setStatusKind('error');
      pushToast({ kind: 'error', message: 'Could not save model roles', detail: message });
    } finally {
      setLoading(false);
    }
  };

  const pullModel = async () => {
    if (!newModel.trim()) return;
    setLoading(true);
    setStatus(`Pulling ${newModel.trim()}… this can take a while.`);
    setStatusKind('info');
    try {
      await invoke('pull_ollama_model', { modelName: newModel.trim() });
      setStatus('Pull completed');
      setStatusKind('ok');
      setNewModel('');
      await loadModels();
    } catch (err: any) {
      const message = err.message || String(err);
      setStatus(`Error: ${message}`);
      setStatusKind('error');
      pushToast({ kind: 'error', message: 'Model download failed', detail: message });
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onClose}
      title="Local models"
      description="Generation runs on this machine through Ollama. Nothing is uploaded."
      size="lg"
      footer={
        <>
          <button
            onClick={onClose}
            className="rounded border border-aether-border bg-aether-bg px-4 py-2 text-body text-aether-text transition hover:bg-aether-surface"
          >
            Close
          </button>
          <button
            onClick={saveConfig}
            disabled={loading}
            className="rounded bg-aether-primary px-4 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
          >
            Save configuration
          </button>
        </>
      }
    >
      <div className="space-y-6">
        <section>
          <div className="mb-2 flex items-center justify-between">
            <h3 className="text-meta font-semibold uppercase tracking-wider text-aether-text-muted">
              Installed models
            </h3>
            <button
              onClick={() => void loadModels()}
              disabled={loading}
              className="rounded border border-aether-border px-2.5 py-1 text-meta text-aether-text transition hover:bg-aether-bg disabled:opacity-50"
            >
              Refresh
            </button>
          </div>
          {models.length === 0 ? (
            <p className="text-body text-aether-text-muted">
              No models installed. Download one below, or run ollama pull in a terminal.
            </p>
          ) : (
            <ul className="divide-y divide-aether-border rounded-lg border border-aether-border">
              {models.map((model) => (
                <li key={model.name} className="flex items-center justify-between gap-3 px-3 py-2">
                  <span className="truncate font-mono text-meta text-aether-text">{model.name}</span>
                  <span className="shrink-0 text-meta text-aether-text-muted">
                    {(model.size / 1024 / 1024 / 1024).toFixed(2)} GB
                  </span>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section>
          <h3 className="mb-2 text-meta font-semibold uppercase tracking-wider text-aether-text-muted">
            Model roles
          </h3>
          <div className="space-y-3">
            <label className="block text-body text-aether-text">
              Fast model
              <span className="ml-2 text-meta text-aether-text-muted">
                extraction, classification
              </span>
              <select
                value={fast}
                onChange={(event) => setFast(event.target.value)}
                className="mt-1 w-full rounded border border-aether-border bg-aether-bg p-2 text-body text-aether-text"
              >
                <option value="">Select a model</option>
                {models.map((model) => (
                  <option key={model.name} value={model.name}>
                    {model.name}
                  </option>
                ))}
              </select>
            </label>

            <label className="block text-body text-aether-text">
              Deep model
              <span className="ml-2 text-meta text-aether-text-muted">tailoring, /apply</span>
              <select
                value={deep}
                onChange={(event) => setDeep(event.target.value)}
                className="mt-1 w-full rounded border border-aether-border bg-aether-bg p-2 text-body text-aether-text"
              >
                <option value="">Select a model</option>
                {models.map((model) => (
                  <option key={model.name} value={model.name}>
                    {model.name}
                  </option>
                ))}
              </select>
            </label>
          </div>
        </section>

        <section>
          <h3 className="mb-2 text-meta font-semibold uppercase tracking-wider text-aether-text-muted">
            Download a model
          </h3>
          <div className="flex flex-wrap gap-2">
            <input
              type="text"
              value={newModel}
              onChange={(event) => setNewModel(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') void pullModel();
              }}
              placeholder="llama3.1:8b"
              className="min-w-[200px] flex-1 rounded border border-aether-border bg-aether-bg p-2 font-mono text-meta text-aether-text outline-none focus:border-aether-primary"
            />
            <button
              onClick={pullModel}
              disabled={loading || !newModel.trim()}
              className="rounded bg-aether-primary px-4 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
            >
              Pull
            </button>
          </div>
          <p className="mt-2 text-meta text-aether-text-muted">
            Model names come from ollama.com/library.
          </p>
        </section>

        {status && (
          <p
            role={statusKind === 'error' ? 'alert' : 'status'}
            className={`rounded border border-aether-border bg-aether-bg p-3 text-meta ${
              statusKind === 'ok'
                ? 'text-aether-success'
                : statusKind === 'error'
                  ? 'text-aether-error'
                  : 'text-aether-text'
            }`}
          >
            {status}
          </p>
        )}
      </div>
    </Dialog>
  );
}
