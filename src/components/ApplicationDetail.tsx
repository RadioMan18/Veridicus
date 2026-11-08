import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle,
  CheckCircle2,
  Compass,
  FileText,
  FolderOpen,
  ShieldCheck,
  XCircle,
} from 'lucide-react';
import Dialog from './Dialog';
import RoleFit, { type PersistedGap } from './RoleFit';
import { pushToast } from './Toaster';

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
    return <div className="p-3 text-meta text-aether-text-muted">Preview unavailable for this page.</div>;
  }
  if (!source) {
    return <div className="p-3 text-meta text-aether-text-muted">Loading page…</div>;
  }
  return (
    <img src={source} alt={alt} className="w-full rounded border border-aether-border bg-white" />
  );
}

function FactCheck({ report }: { report: any }) {
  if (!report) {
    return <p className="text-meta text-aether-text-muted">No fact-check report was produced.</p>;
  }

  const blocking: string[] = [
    ...(report.never_claim_violations || []),
    ...(report.unverified_metrics || []),
    ...(report.qualifier_violations || []),
  ];
  const advisory: string[] = [
    ...(report.claim_coverage_warnings || []),
    ...(report.employer_consistency_warnings || []),
    ...(report.ask_user_clarifications || []),
  ];

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-1.5">
        <span
          className={`rounded-full border px-2 py-0.5 text-meta ${
            report.blocking
              ? 'border-aether-error text-aether-error'
              : 'border-aether-success text-aether-success'
          }`}
        >
          {report.blocking ? 'Blocking issues' : 'No blocking issues'}
        </span>
        <span className="rounded-full border border-aether-border px-2 py-0.5 text-meta text-aether-text-muted">
          {report.metrics_verified}/{report.metrics_found} metrics verified
        </span>
        {advisory.length > 0 && (
          <span className="rounded-full border border-aether-warning px-2 py-0.5 text-meta text-aether-warning">
            {advisory.length} advisory
          </span>
        )}
      </div>

      {blocking.map((finding, index) => (
        <p key={`blocking-${index}`} className="flex gap-2 text-meta text-aether-error">
          <XCircle size={14} className="mt-0.5 shrink-0" />
          <span>{finding}</span>
        </p>
      ))}
      {advisory.map((finding, index) => (
        <p key={`advisory-${index}`} className="flex gap-2 text-meta text-aether-warning">
          <AlertTriangle size={14} className="mt-0.5 shrink-0" />
          <span>{finding}</span>
        </p>
      ))}
    </div>
  );
}

function Guardrails({ report }: { report: any }) {
  if (!report) return null;
  const findings: any[] = report.findings || [];
  if (findings.length === 0) {
    return <p className="text-meta text-aether-success">Honesty guardrails passed.</p>;
  }
  return (
    <div className="space-y-1">
      {findings.map((finding, index) => (
        <p
          key={`${finding.code}-${index}`}
          className={`flex gap-2 text-meta ${finding.blocking ? 'text-aether-error' : 'text-aether-warning'}`}
        >
          {finding.blocking ? (
            <XCircle size={14} className="mt-0.5 shrink-0" />
          ) : (
            <AlertTriangle size={14} className="mt-0.5 shrink-0" />
          )}
          <span>
            <span className="font-mono">{finding.code}</span> · {finding.message}
          </span>
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
    return <p className="text-meta text-aether-success">Layout checks passed.</p>;
  }
  return (
    <div className="space-y-1">
      {items.map((item, index) => (
        <p key={`flag-${index}`} className="flex gap-2 text-meta text-aether-warning">
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
        <h3 className="flex items-center gap-2 text-row font-semibold text-aether-text">
          <FileText size={16} className="text-aether-primary-ink" />
          {title}
        </h3>
        <span
          className={`text-meta ${compiled?.success ? 'text-aether-success' : 'text-aether-error'}`}
        >
          {compiled?.success ? `${raster?.page_count ?? pages.length} page(s)` : 'Not compiled'}
        </span>
      </header>

      <FactCheck report={report} />
      <VisualFlags flags={flags} />

      <div className="max-h-[380px] space-y-2 overflow-y-auto rounded-lg bg-aether-bg p-2">
        {pages.length === 0 && (
          <p className="p-3 text-meta text-aether-text-muted">No rendered pages available.</p>
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
  const [blocked, setBlocked] = useState<string | null>(null);
  const [overrideReason, setOverrideReason] = useState('');
  const [problemNote, setProblemNote] = useState('');
  const [problemSent, setProblemSent] = useState(false);
  const [incidentCategory, setIncidentCategory] = useState('other');
  const [incidentId, setIncidentId] = useState<string | null>(null);
  const [disclosureAcked, setDisclosureAcked] = useState(false);
  const [exportingIncidents, setExportingIncidents] = useState(false);
  const [gaps, setGaps] = useState<PersistedGap[]>([]);
  const [approvedWording, setApprovedWording] = useState('');
  const [savingWording, setSavingWording] = useState(false);
  const folder = folderOf(result);

  const loadGaps = async () => {
    try {
      const rows = await invoke<PersistedGap[]>('list_gaps', {
        applicationId: result.application_id,
      });
      setGaps(rows);
    } catch {
      // A run that failed before persisting gaps has no matrix to show.
    }
  };

  useEffect(() => {
    void loadGaps();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [result.application_id]);

  useEffect(() => {
    invoke<any[]>('get_application_records')
      .then((rows) => {
        const row = Array.isArray(rows)
          ? rows.find((entry) => entry.id === result.application_id)
          : null;
        if (row) setApprovedWording(row.approved_wording || '');
      })
      .catch(() => {});
  }, [result.application_id]);

  const saveWording = async () => {
    if (!approvedWording.trim()) {
      pushToast({ kind: 'error', message: 'Approved wording cannot be empty' });
      return;
    }
    setSavingWording(true);
    try {
      await invoke('record_approved_wording', {
        applicationId: result.application_id,
        wording: approvedWording.trim(),
      });
      pushToast({ kind: 'success', message: 'Approved wording saved for reuse' });
    } catch (error: any) {
      pushToast({
        kind: 'error',
        message: 'Could not save approved wording',
        detail: error?.message || String(error),
      });
    } finally {
      setSavingWording(false);
    }
  };

  const openFolder = async () => {
    if (!folder) return;
    setFolderError(null);
    try {
      await invoke('open_application_folder', { path: folder });
    } catch (error: any) {
      const message = error?.message || String(error);
      setFolderError(message);
      pushToast({ kind: 'error', message: 'Could not open the folder', detail: message });
    }
  };

  const approve = async (override?: string) => {
    if (!disclosureAcked) {
      setApproveError('Acknowledge the AI-assisted disclosure before approving.');
      return;
    }
    setApproving(true);
    setApproveError(null);
    try {
      const oid = await invoke<string>('commit_application', {
        applicationId: result.application_id,
        overrideReason: override?.trim() ? override.trim() : null,
      });
      setCommit(oid);
      setBlocked(null);
      pushToast({ kind: 'success', message: 'Materials approved and committed', detail: oid });
    } catch (error: any) {
      const message = error?.message || String(error);
      setApproveError(message);
      if (message.includes('Blocked by honesty guardrails')) {
        setBlocked(message);
      }
      pushToast({ kind: 'error', message: 'Commit failed', detail: message });
    } finally {
      setApproving(false);
    }
  };

  const reportProblem = async () => {
    try {
      const report: any = await invoke('report_output_problem', {
        applicationId: result.application_id,
        category: incidentCategory,
        note: problemNote,
      });
      setIncidentId(report.incident_id);
      setProblemSent(true);
      pushToast({ kind: 'success', message: 'Incident recorded' });
    } catch (error: any) {
      const message = error?.message || String(error);
      pushToast({ kind: 'error', message: 'Could not record the report', detail: message });
    }
  };

  const acknowledgeDisclosure = async (checked: boolean) => {
    if (!checked) {
      setDisclosureAcked(false);
      return;
    }
    try {
      await invoke('acknowledge_output_disclosure', {
        applicationId: result.application_id,
      });
      setDisclosureAcked(true);
    } catch (error: any) {
      setDisclosureAcked(false);
      pushToast({
        kind: 'error',
        message: 'Could not record the acknowledgement',
        detail: error?.message || String(error),
      });
    }
  };

  const exportIncidents = async () => {
    setExportingIncidents(true);
    try {
      const path = await invoke<string | null>('export_incident_log');
      if (path) {
        pushToast({ kind: 'success', message: 'Incident log exported', detail: path });
      }
    } catch (error: any) {
      pushToast({
        kind: 'error',
        message: 'Could not export the incident log',
        detail: error?.message || String(error),
      });
    } finally {
      setExportingIncidents(false);
    }
  };

  return (
    <Dialog
      isOpen
      onClose={onClose}
      title={`${result.employer || 'Unknown employer'} — ${result.role || 'Unknown role'}`}
      description={`Application ${result.application_id}`}
      size="xl"
      bodyClassName="lg:px-6"
      actions={
        <>
          <span
            className={`rounded-full border px-2 py-1 text-meta ${
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
            className="flex items-center gap-2 rounded border border-aether-border px-3 py-1.5 text-meta text-aether-text transition hover:bg-aether-primary/10 disabled:opacity-50"
          >
            <FolderOpen size={14} /> Open folder
          </button>
        </>
      }
      footer={
        <>
          <div className="mr-auto min-w-0 text-meta text-aether-text-muted">
            {result.error_message ? (
              <span className="whitespace-pre-wrap text-aether-warning">{result.error_message}</span>
            ) : commit ? (
              <span className="text-aether-success">Committed {commit.slice(0, 8)}</span>
            ) : (
              <span>Review both documents and their findings, then approve.</span>
            )}
            {folderError && <span className="ml-2 text-aether-error">{folderError}</span>}
            {approveError && <span className="ml-2 text-aether-error">{approveError}</span>}
          </div>

          {!commit && (
            <button
              onClick={() => approve()}
              disabled={approving || !result.success || !disclosureAcked}
              className="flex items-center gap-2 rounded bg-aether-primary px-4 py-2 text-body font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
            >
              <ShieldCheck size={16} />
              {approving ? 'Committing…' : 'Approve materials'}
            </button>
          )}
          {commit && (
            <span className="flex items-center gap-2 text-body text-aether-success">
              <CheckCircle2 size={16} /> Approved and committed
            </span>
          )}
        </>
      }
    >
      <section className="mb-4 rounded-xl border border-aether-border bg-aether-surface p-4">
        <h3 className="mb-2 flex items-center gap-2 text-row font-semibold text-aether-text">
          <ShieldCheck size={16} className="text-aether-primary-ink" />
          Honesty guardrails
        </h3>
        <Guardrails report={result.guardrail_report} />
        <label className="mt-3 flex cursor-pointer items-start gap-2 rounded-lg border border-aether-border bg-aether-bg p-3 text-meta text-aether-text-muted">
          <input
            type="checkbox"
            checked={disclosureAcked}
            onChange={(event) => acknowledgeDisclosure(event.target.checked)}
            className="mt-0.5 h-4 w-4 shrink-0 accent-aether-primary"
          />
          <span>
            <span className="block font-medium text-aether-text">AI-assisted draft</span>
            Verified against your confirmed claims; you remain responsible for submission. This
            acknowledgement is recorded in the application audit trail.
          </span>
        </label>
        {(result.resume_tailored?.model || result.cover_letter?.model) && (
          <p className="mt-1 text-meta text-aether-text-muted">
            Model {result.resume_tailored?.model || result.cover_letter?.model} · digest{' '}
            {(result.resume_tailored?.model_digest || result.cover_letter?.model_digest || 'unavailable').slice(0, 12)} · template v
            {result.resume_tailored?.template_version ?? result.cover_letter?.template_version ?? '?'}
          </p>
        )}
        <p className="mt-1 text-meta text-aether-text-muted">
          Decision record: audit/events.jsonl in the application folder.
        </p>
        <div className="mt-3 border-t border-aether-border pt-3">
          <p className="mb-1 text-meta font-medium text-aether-text">Something wrong with this output?</p>
          {problemSent ? (
            <p className="text-meta text-aether-success">
              Incident recorded{incidentId ? ` (${incidentId.slice(0, 8)})` : ''}.
            </p>
          ) : (
            <div className="flex flex-col gap-2 sm:flex-row">
              <select
                value={incidentCategory}
                onChange={(event) => setIncidentCategory(event.target.value)}
                aria-label="Incident category"
                className="rounded border border-aether-border bg-aether-bg px-2 py-1.5 text-meta text-aether-text outline-none focus:border-aether-primary"
              >
                <option value="false_block">False block</option>
                <option value="missed_false_claim">Missed false claim</option>
                <option value="scope_error">Scope error</option>
                <option value="wrong_evidence">Wrong evidence</option>
                <option value="other">Other</option>
              </select>
              <input
                value={problemNote}
                onChange={(event) => setProblemNote(event.target.value)}
                placeholder="What looks wrong?"
                className="min-w-0 flex-1 rounded border border-aether-border bg-aether-bg px-3 py-1.5 text-meta text-aether-text outline-none focus:border-aether-primary"
              />
              <button
                onClick={reportProblem}
                disabled={!problemNote.trim()}
                className="rounded border border-aether-border px-3 py-1.5 text-meta text-aether-text transition hover:bg-aether-primary/10 disabled:opacity-50"
              >
                Report
              </button>
            </div>
          )}
          <button
            onClick={exportIncidents}
            disabled={exportingIncidents}
            className="mt-2 rounded border border-aether-border px-3 py-1.5 text-meta text-aether-text-muted transition hover:bg-aether-primary/10 disabled:opacity-50"
          >
            {exportingIncidents ? 'Exporting…' : 'Export incident log'}
          </button>
        </div>
        {blocked && !commit && (
          <div className="mt-3 space-y-2 rounded-lg border border-aether-error/40 bg-aether-bg p-3">
            <p className="whitespace-pre-wrap text-meta text-aether-error">{blocked}</p>
            <textarea
              value={overrideReason}
              onChange={(event) => setOverrideReason(event.target.value)}
              placeholder="Why are you overriding the guardrails? This is recorded with the application."
              rows={3}
              className="w-full rounded border border-aether-border bg-aether-surface px-3 py-2 text-meta text-aether-text outline-none focus:border-aether-primary"
            />
            <button
              onClick={() => approve(overrideReason)}
              disabled={approving || !overrideReason.trim() || !disclosureAcked}
              className="rounded border border-aether-error px-3 py-1.5 text-meta text-aether-error transition hover:bg-aether-error/10 disabled:opacity-50"
            >
              Commit with override
            </button>
          </div>
        )}
      </section>

      <section className="mb-4 rounded-xl border border-aether-border bg-aether-surface p-4">
        <h3 className="mb-3 flex items-center gap-2 text-row font-semibold text-aether-text">
          <Compass size={16} className="text-aether-primary-ink" />
          Role fit
        </h3>
        <RoleFit analysis={result.gap_analysis ?? null} gaps={gaps} onDecision={loadGaps} />
      </section>

      <section className="mb-4 rounded-xl border border-aether-border bg-aether-surface p-4">
        <h3 className="mb-2 text-row font-semibold text-aether-text">Approved wording</h3>
        <p className="mb-2 text-meta text-aether-text-muted">
          The confirmed wording you approve for reuse on this application. It is stored on the
          record and compared when the employer and role are checked.
        </p>
        <textarea
          value={approvedWording}
          onChange={(event) => setApprovedWording(event.target.value)}
          rows={2}
          placeholder="e.g. Cut reporting costs by 12% for the division"
          className="w-full rounded border border-aether-border bg-aether-bg p-2 text-body text-aether-text"
          aria-label="Approved wording"
        />
        <div className="mt-2 flex justify-end">
          <button
            onClick={saveWording}
            disabled={savingWording}
            className="rounded border border-aether-border px-3 py-1.5 text-meta text-aether-text transition hover:bg-aether-bg disabled:opacity-50"
          >
            {savingWording ? 'Saving…' : 'Save wording'}
          </button>
        </div>
      </section>

      <div className="grid gap-4 lg:grid-cols-2">
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
    </Dialog>
  );
}
