import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle,
  CheckCircle2,
  FileText,
  FolderOpen,
  ShieldCheck,
  XCircle,
} from 'lucide-react';

interface ApplicationDetailProps {
  result: any;
  onClose: () => void;
}

function folderOf(result: any): string | null {
  const path: string | undefined =
    result.jd_path ||
    result.resume_pdf_compiled?.pdf_path ||
    result.pdf_compiled?.pdf_path;
  if (!path) return null;
  const index = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'));
  return index > 0 ? path.slice(0, index) : null;
}

function PagePreview({ path, alt }: { path: string; alt: string }) {
  const [source, setSource] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    invoke<string>('get_artifact_preview', { path })
      .then((dataUrl) => {
        if (active) setSource(dataUrl);
      })
      .catch(() => {
        if (active) setFailed(true);
      });
    return () => {
      active = false;
    };
  }, [path]);

  if (failed) {
    return <div className="p-3 text-xs text-aether-text-muted">Preview unavailable for this page.</div>;
  }
  if (!source) {
    return <div className="p-3 text-xs text-aether-text-muted">Loading page…</div>;
  }
  return (
    <img
      src={source}
      alt={alt}
      className="w-full rounded border border-aether-border bg-white"
    />
  );
}

function FactCheck({ report }: { report: any }) {
  if (!report) {
    return <p className="text-xs text-aether-text-muted">No fact-check report was produced.</p>;
  }

  const blocking: string[] = [
    ...(report.never_claim_violations || []),
    ...(report.scope_violations || []),
  ];
  const advisory: string[] = [
    ...(report.unverified_metrics || []),
    ...(report.employer_consistency_warnings || []),
    ...(report.ask_user_clarifications || []),
  ];

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-1.5 text-xs">
        <span
          className={`rounded-full border px-2 py-0.5 ${
            report.blocking
              ? 'border-aether-error text-aether-error'
              : 'border-aether-success text-aether-success'
          }`}
        >
          {report.blocking ? 'Blocking issues' : 'No blocking issues'}
        </span>
        <span className="rounded-full border border-aether-border px-2 py-0.5 text-aether-text-muted">
          {report.metrics_verified}/{report.metrics_found} metrics verified
        </span>
        {advisory.length > 0 && (
          <span className="rounded-full border border-aether-warning px-2 py-0.5 text-aether-warning">
            {advisory.length} advisory
          </span>
        )}
      </div>

      {blocking.map((finding, index) => (
        <p key={`blocking-${index}`} className="flex gap-2 text-xs text-aether-error">
          <XCircle size={14} className="mt-0.5 shrink-0" />
          <span>{finding}</span>
        </p>
      ))}
      {advisory.map((finding, index) => (
        <p key={`advisory-${index}`} className="flex gap-2 text-xs text-aether-warning">
          <AlertTriangle size={14} className="mt-0.5 shrink-0" />
          <span>{finding}</span>
        </p>
      ))}
    </div>
  );
}

function VisualFlags({ flags }: { flags: any }) {
  if (!flags) return null;
  const items: string[] = [
    ...(flags.metadata_flags || []),
    ...(flags.overflow_flags || []),
    ...(flags.orphan_heading_flags || []),
    ...(flags.broken_wrap_flags || []),
    ...(flags.placeholder_flags || []),
  ];
  if (items.length === 0) {
    return <p className="text-xs text-aether-success">Layout checks passed.</p>;
  }
  return (
    <div className="space-y-1">
      {items.map((item, index) => (
        <p key={`flag-${index}`} className="flex gap-2 text-xs text-aether-warning">
          <AlertTriangle size={14} className="mt-0.5 shrink-0" />
          <span>{item}</span>
        </p>
      ))}
    </div>
  );
}

function DocumentPanel({
  title,
  compiled,
  raster,
  report,
  flags,
}: {
  title: string;
  compiled: any;
  raster: any;
  report: any;
  flags: any;
}) {
  const pages: string[] = raster?.png_paths || [];

  return (
    <section className="flex min-w-0 flex-col gap-3 rounded-xl border border-aether-border bg-aether-surface p-4">
      <header className="flex items-center justify-between gap-2">
        <h3 className="flex items-center gap-2 text-sm font-semibold text-aether-text">
          <FileText size={16} className="text-aether-primary" />
          {title}
        </h3>
        <span
          className={`text-xs ${compiled?.success ? 'text-aether-success' : 'text-aether-error'}`}
        >
          {compiled?.success ? `${raster?.page_count ?? pages.length} page(s)` : 'Not compiled'}
        </span>
      </header>

      <FactCheck report={report} />
      <VisualFlags flags={flags} />

      <div className="max-h-[380px] space-y-2 overflow-y-auto rounded-lg bg-aether-bg p-2">
        {pages.length === 0 && (
          <p className="p-3 text-xs text-aether-text-muted">No rendered pages available.</p>
        )}
        {pages.map((page, index) => (
          <PagePreview key={page} path={page} alt={`${title} page ${index + 1}`} />
        ))}
      </div>
    </section>
  );
}

export default function ApplicationDetail({ result, onClose }: ApplicationDetailProps) {
  const [commit, setCommit] = useState<string | null>(result.git_commit ?? null);
  const [approving, setApproving] = useState(false);
  const [approveError, setApproveError] = useState<string | null>(null);
  const [folderError, setFolderError] = useState<string | null>(null);
  const folder = folderOf(result);

  const openFolder = async () => {
    if (!folder) return;
    setFolderError(null);
    try {
      await invoke('open_application_folder', { path: folder });
    } catch (error: any) {
      setFolderError(error?.message || String(error));
    }
  };

  const approve = async () => {
    setApproving(true);
    setApproveError(null);
    try {
      const oid = await invoke<string>('commit_application', {
        applicationId: result.application_id,
      });
      setCommit(oid);
    } catch (error: any) {
      setApproveError(error?.message || String(error));
    } finally {
      setApproving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm">
      <div className="flex max-h-full w-full max-w-6xl flex-col overflow-hidden rounded-xl border border-aether-border bg-aether-surface shadow-2xl">
        <header className="flex flex-wrap items-start justify-between gap-3 border-b border-aether-border p-4">
          <div>
            <h2 className="text-lg font-bold text-aether-text">
              {result.employer || 'Unknown employer'} — {result.role || 'Unknown role'}
            </h2>
            <p className="mt-1 font-mono text-xs text-aether-text-muted">
              {result.application_id}
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <span
              className={`rounded-full border px-2 py-1 text-xs ${
                result.success
                  ? 'border-aether-success text-aether-success'
                  : 'border-aether-error text-aether-error'
              }`}
            >
              {result.success ? 'Materials generated' : 'Run failed'}
            </span>
            <button
              onClick={openFolder}
              disabled={!folder}
              className="flex items-center gap-2 rounded border border-aether-border px-3 py-1.5 text-xs text-aether-text transition hover:bg-aether-primary/10 disabled:opacity-50"
            >
              <FolderOpen size={14} /> Open folder
            </button>
            <button
              onClick={onClose}
              className="rounded border border-aether-border px-3 py-1.5 text-xs text-aether-text-muted transition hover:text-aether-text"
            >
              Close
            </button>
          </div>
        </header>

        <div className="grid flex-1 gap-4 overflow-y-auto p-4 lg:grid-cols-2">
          <DocumentPanel
            title="Resume"
            compiled={result.resume_pdf_compiled}
            raster={result.resume_pages_rasterized}
            report={result.resume_fact_check_report}
            flags={result.resume_visual_flags}
          />
          <DocumentPanel
            title="Cover letter"
            compiled={result.pdf_compiled}
            raster={result.pages_rasterized}
            report={result.fact_check_report}
            flags={result.visual_flags}
          />
        </div>

        <footer className="flex flex-wrap items-center justify-between gap-3 border-t border-aether-border p-4">
          <div className="min-w-0 text-xs text-aether-text-muted">
            {result.error_message ? (
              <span className="whitespace-pre-wrap text-aether-warning">{result.error_message}</span>
            ) : commit ? (
              <span className="text-aether-success">Committed {commit.slice(0, 8)}</span>
            ) : (
              <span>
                Nothing is committed yet. Review both documents and their findings, then approve.
              </span>
            )}
            {folderError && <span className="ml-2 text-aether-error">{folderError}</span>}
            {approveError && <span className="ml-2 text-aether-error">{approveError}</span>}
          </div>

          {!commit && (
            <button
              onClick={approve}
              disabled={approving || !result.success}
              className="flex items-center gap-2 rounded bg-aether-primary px-4 py-2 text-sm font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
            >
              <ShieldCheck size={16} />
              {approving ? 'Committing…' : 'Approve materials'}
            </button>
          )}
          {commit && (
            <span className="flex items-center gap-2 text-sm text-aether-success">
              <CheckCircle2 size={16} /> Approved and committed
            </span>
          )}
        </footer>
      </div>
    </div>
  );
}
