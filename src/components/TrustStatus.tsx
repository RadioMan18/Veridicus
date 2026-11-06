import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface TrustReport {
  ollama_online: boolean;
  model: string | null;
  last_commit_seconds: number | null;
}

function relativeAge(seconds: number | null) {
  if (seconds === null) return null;
  if (seconds < 60) return 'just now';
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.round(hours / 24)}d ago`;
}

/**
 * Quiet trust line: proves the app is local, names the model in use, and shows how
 * recent the last commit is. Also surfaces Ollama being down before a model call fails.
 */
export default function TrustStatus() {
  const [report, setReport] = useState<TrustReport | null>(null);

  useEffect(() => {
    let active = true;

    const load = async () => {
      try {
        const next = await invoke<TrustReport>('get_trust_status');
        if (active) setReport(next);
      } catch {
        // The indicator is informative; a failed probe should not surface as an error.
      }
    };

    void load();
    const timer = window.setInterval(() => void load(), 30_000);

    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, []);

  if (!report) return null;

  const commit = relativeAge(report.last_commit_seconds);

  return (
    <div
      className="flex shrink-0 items-center gap-1.5 text-meta text-aether-text-muted"
      title={
        report.ollama_online
          ? 'Generation runs against Ollama on this machine. Nothing leaves the device.'
          : 'Ollama is not responding on localhost:11434. Model calls will fail until it is running.'
      }
    >
      <span
        className={`h-1.5 w-1.5 rounded-full ${
          report.ollama_online ? 'bg-aether-success' : 'bg-aether-warning'
        }`}
      />
      <span className="hidden sm:inline">Local only</span>
      <span className="hidden sm:inline">·</span>
      <span className={report.ollama_online ? '' : 'text-aether-warning'}>
        {report.ollama_online ? report.model || 'Ollama ready' : 'Ollama offline'}
      </span>
      {commit && (
        <>
          <span className="hidden xl:inline">·</span>
          <span className="hidden xl:inline">last commit {commit}</span>
        </>
      )}
    </div>
  );
}
