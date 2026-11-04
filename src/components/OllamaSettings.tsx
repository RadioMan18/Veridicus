import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

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
  const [status, setStatus] = useState<string>('');
  const [selectedFast, setSelectedFast] = useState<string>('');
  const [selectedDeep, setSelectedDeep] = useState<string>('');
  const [newModel, setNewModel] = useState<string>('');

  const loadModels = async () => {
    setLoading(true);
    setStatus('Loading models...');
    try {
      const result: OllamaModel[] = await invoke('get_ollama_models');
      setModels(result);
      setStatus(result.length > 0 ? `Found ${result.length} models` : 'No models found');

      try {
        const config: any = await invoke('get_ollama_config');
        if (config.fast_model) setSelectedFast(config.fast_model);
        if (config.deep_model) setSelectedDeep(config.deep_model);
      } catch (e) {
        // ignore
      }
    } catch (err: any) {
      setStatus('Error: ' + (err.message || String(err)));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (isOpen) {
      loadModels();
    }
  }, [isOpen]);

  const saveConfig = async () => {
    setLoading(true);
    setStatus('Saving configuration...');
    try {
      await invoke('save_ollama_config', {
        fast_model: selectedFast || null,
        deep_model: selectedDeep || null,
      });
      setStatus('✓ Configuration saved');
    } catch (err: any) {
      setStatus('Error: ' + (err.message || String(err)));
    } finally {
      setLoading(false);
    }
  };

  const pullModel = async () => {
    if (!newModel.trim()) return;
    setLoading(true);
    setStatus('Pulling ' + newModel.trim() + '... This may take a while.');
    try {
      await invoke('pull_ollama_model', { modelName: newModel.trim() });
      setStatus('✓ Pull completed');
      setNewModel('');
      await loadModels();
    } catch (err: any) {
      setStatus('Error: ' + (err.message || String(err)));
    } finally {
      setLoading(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 z-50">
      <div className="bg-aether-surface border border-aether-border rounded-xl w-full max-w-3xl overflow-hidden shadow-2xl max-h-[90vh] overflow-y-auto">
        <div className="p-4 border-b border-aether-border flex items-center justify-between">
          <h2 className="text-xl font-bold text-aether-text">Ollama Settings</h2>
          <button onClick={onClose} className="text-aether-text-muted hover:text-aether-text">✕</button>
        </div>

        <div className="p-6 space-y-6">
          <div>
            <h3 className="font-semibold text-aether-text mb-3">Installed Models</h3>
            {models.length === 0 ? (
              <p className="text-aether-text-muted text-sm">No models installed</p>
            ) : (
              <div className="space-y-2">
                {models.map((model) => (
                  <div key={model.name} className="p-3 bg-aether-bg border border-aether-border rounded flex items-center justify-between">
                    <div>
                      <p className="text-aether-text font-medium">{model.name}</p>
                      <p className="text-xs text-aether-text-muted">{(model.size / 1024 / 1024 / 1024).toFixed(2)} GB</p>
                    </div>
                  </div>
                ))}
              </div>
            )}
            <button onClick={loadModels} disabled={loading} className="mt-3 px-3 py-1 bg-aether-primary text-aether-primary-fg hover:bg-aether-primary-hover text-sm rounded transition disabled:opacity-50">
              Refresh
            </button>
          </div>

          <div className="border-t border-aether-border pt-6">
            <h3 className="font-semibold text-aether-text mb-3">Model Roles</h3>
            <div className="space-y-4">
              <div>
                <label className="block text-sm text-aether-text mb-2">Fast Model (quick tasks)</label>
                <select
                  value={selectedFast}
                  onChange={(e) => setSelectedFast(e.target.value)}
                  className="w-full p-2 bg-aether-bg border border-aether-border rounded text-aether-text"
                >
                  <option value="">Select a model</option>
                  {models.map((model) => (
                    <option key={model.name} value={model.name}>{model.name}</option>
                  ))}
                </select>
              </div>
              <div>
                <label className="block text-sm text-aether-text mb-2">Deep Model (complex tasks like /apply)</label>
                <select
                  value={selectedDeep}
                  onChange={(e) => setSelectedDeep(e.target.value)}
                  className="w-full p-2 bg-aether-bg border border-aether-border rounded text-aether-text"
                >
                  <option value="">Select a model</option>
                  {models.map((model) => (
                    <option key={model.name} value={model.name}>{model.name}</option>
                  ))}
                </select>
              </div>
              <button onClick={saveConfig} disabled={loading} className="px-4 py-2 bg-aether-selected text-aether-selected-fg hover:bg-aether-selected-border rounded transition disabled:opacity-50">
                Save Configuration
              </button>
            </div>
          </div>

          <div className="border-t border-aether-border pt-6">
            <h3 className="font-semibold text-aether-text mb-3">Download New Model</h3>
            <div className="flex gap-2">
              <input
                type="text"
                value={newModel}
                onChange={(e) => setNewModel(e.target.value)}
                placeholder="e.g., llama3.1:8b, mistral:7b"
                className="flex-1 p-2 bg-aether-bg border border-aether-border rounded text-aether-text"
              />
              <button onClick={pullModel} disabled={loading || !newModel.trim()} className="px-4 py-2 bg-aether-warning text-aether-primary-fg hover:bg-aether-warning-hover rounded transition disabled:opacity-50">
                Pull Model
              </button>
            </div>
            <p className="text-xs text-aether-text-muted mt-2">Visit ollama.com/library for available models</p>
          </div>

          {status && (
            <div className="p-3 rounded bg-aether-bg border border-aether-border text-sm">
              <p className={status.startsWith('✓') ? 'text-aether-success' : status.startsWith('Error') ? 'text-aether-error' : 'text-aether-text'}>{status}</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
