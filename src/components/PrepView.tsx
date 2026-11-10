import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import Markdown from './Markdown';
import { withCompletionChime } from '../lib/completionSound';
import { MockHistoryStrip, type MockSessionRecord } from './MockInterview';

interface InterviewQuestion {
  question: string;
  group: string;
  evidence: string;
  claim_ids: string[];
  gap_ids: string[];
}

interface StarStory {
  theme: string;
  status: string;
  situation: string;
  task: string;
  action: string;
  result: string;
  claim_ids: string[];
  gap_ids: string[];
  reason: string | null;
}

interface InterviewPrep {
  application_id: string;
  employer: string;
  role: string;
  questions: InterviewQuestion[];
  stories: StarStory[];
  warnings: string[];
}

interface ApplicationRecord {
  id: string;
  employer: string;
  role: string;
}

type Tab = 'briefing' | 'questions' | 'stories';

const GROUPS: [string, string][] = [
  ['technical', 'Technical'],
  ['behavioral', 'Behavioral'],
  ['culture-fit', 'Culture fit'],
];

export default function PrepView({ onOpenFit }: { onOpenFit?: () => void }) {
  const [tab, setTab] = useState<Tab>('briefing');

  const [company, setCompany] = useState('');
  const [output, setOutput] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [records, setRecords] = useState<ApplicationRecord[]>([]);
  const [appId, setAppId] = useState('');
  const [prep, setPrep] = useState<InterviewPrep | null>(null);
  const [prepError, setPrepError] = useState<string | null>(null);
  const [prepLoading, setPrepLoading] = useState(false);
  const [mockSessions, setMockSessions] = useState<MockSessionRecord[]>([]);

  useEffect(() => {
    invoke<ApplicationRecord[]>('get_application_records')
      .then((rows) => {
        const list = Array.isArray(rows) ? rows : [];
        setRecords(list);
        if (list.length > 0) setAppId((current) => current || list[0].id);
      })
      .catch(() => setRecords([]));
  }, []);

  useEffect(() => {
    if (!appId) {
      setPrep(null);
      return;
    }
    let active = true;
    invoke<InterviewPrep>('get_interview_prep', { applicationId: appId })
      .then((value) => {
        if (active) {
          setPrep(value);
          setPrepError(null);
        }
      })
      .catch(() => {
        if (active) setPrep(null);
      });
    return () => {
      active = false;
    };
  }, [appId]);

  // The runs practising for this application, so the prep view shows what was
  // already asked and what the weakest dimension was.
  useEffect(() => {
    if (!appId) {
      setMockSessions([]);
      return;
    }
    let active = true;
    invoke<MockSessionRecord[]>('recent_mock_sessions', { applicationId: appId })
      .then((rows) => {
        if (active) setMockSessions(Array.isArray(rows) ? rows : []);
      })
      .catch(() => {
        if (active) setMockSessions([]);
      });
    return () => {
      active = false;
    };
  }, [appId]);

  const run = async () => {
    const name = company.trim();
    if (!name) {
      setError('Enter a company name first.');
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const briefing: string = await withCompletionChime(
        () => invoke<string>('run_company_research', { company: name }),
        { label: 'Company briefing' },
      );
      setOutput(briefing);
    } catch (err: any) {
      setError(err?.message || String(err));
    } finally {
      setLoading(false);
    }
  };

  const generate = async () => {
    if (!appId) return;
    setPrepLoading(true);
    setPrepError(null);
    try {
      const value = await withCompletionChime(
        () =>
          invoke<InterviewPrep>('generate_interview_prep', {
            applicationId: appId,
          }),
        { label: 'Interview prep' },
      );
      setPrep(value);
    } catch (err: any) {
      setPrepError(err?.message || String(err));
    } finally {
      setPrepLoading(false);
    }
  };

  const tabs: [Tab, string][] = [
    ['briefing', 'Briefing'],
    ['questions', 'Question bank'],
    ['stories', 'STAR bank'],
  ];

  return (
    <div className="max-w-3xl space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="text-section text-aether-text">Prep</h2>
          <p className="text-body text-aether-text-muted">
            Research the company, rehearse the questions, and know which stories you can support.
          </p>
        </div>
        {onOpenFit && (
          <button
            onClick={onOpenFit}
            className="rounded-lg border border-aether-border px-3 py-2 text-meta text-aether-text transition hover:bg-aether-surface"
          >
            Open role fit
          </button>
        )}
      </div>

      <div className="flex gap-1 border-b border-aether-border" role="tablist">
        {tabs.map(([id, label]) => (
          <button
            key={id}
            role="tab"
            aria-selected={tab === id}
            onClick={() => setTab(id)}
            className={`border-b-2 px-3 py-2 text-body transition ${
              tab === id
                ? 'border-aether-primary text-aether-text'
                : 'border-transparent text-aether-text-muted hover:text-aether-text'
            }`}
          >
            {label}
          </button>
        ))}
      </div>

      {tab === 'briefing' && (
        <>
          <div className="flex flex-wrap gap-2">
            <input
              value={company}
              onChange={(event) => setCompany(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') void run();
              }}
              placeholder="Company name, e.g. NetDocuments"
              className="min-w-[240px] flex-1 rounded-lg border border-aether-border bg-aether-surface px-3 py-2 text-sm outline-none focus:border-aether-primary"
            />
            <button
              onClick={run}
              disabled={loading}
              className="rounded-lg bg-aether-primary px-4 py-2 text-sm font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
            >
              {loading ? 'Researching…' : 'Research'}
            </button>
          </div>

          {error && <p className="text-body text-aether-error">{error}</p>}

          {output ? (
            <section className="rounded-xl border border-aether-border bg-aether-surface p-6">
              <Markdown content={output} />
            </section>
          ) : (
            <p className="text-body text-aether-text-muted">
              Nothing yet. Enter a company to generate a briefing with recent context, likely
              interview themes and talking points.
            </p>
          )}
        </>
      )}

      {tab !== 'briefing' && (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={appId}
              onChange={(event) => setAppId(event.target.value)}
              className="min-w-[240px] flex-1 rounded-lg border border-aether-border bg-aether-surface px-3 py-2 text-sm text-aether-text outline-none focus:border-aether-primary"
              aria-label="Application"
            >
              {records.length === 0 && <option value="">No applications yet</option>}
              {records.map((record) => (
                <option key={record.id} value={record.id}>
                  {record.employer} — {record.role}
                </option>
              ))}
            </select>
            <button
              onClick={generate}
              disabled={prepLoading || !appId}
              className="rounded-lg bg-aether-primary px-4 py-2 text-sm font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover disabled:opacity-50"
            >
              {prepLoading ? 'Preparing…' : prep ? 'Regenerate' : 'Generate prep'}
            </button>
          </div>

          <MockHistoryStrip sessions={mockSessions} title="Mock interviews for this role" />

          {prepError && <p className="text-body text-aether-error">{prepError}</p>}

          {prep && prep.warnings.length > 0 && (
            <ul className="space-y-1 rounded-lg border border-aether-border bg-aether-surface p-3 text-meta text-aether-text-muted">
              {prep.warnings.map((warning, index) => (
                <li key={index}>{warning}</li>
              ))}
            </ul>
          )}

          {!prep && !prepLoading && (
            <p className="text-body text-aether-text-muted">
              No prep yet for this application. Generate it to build the question bank from the
              posting and the STAR bank from your confirmed claims.
            </p>
          )}

          {prep && tab === 'questions' && (
            <div className="space-y-4">
              {GROUPS.map(([group, label]) => {
                const questions = prep.questions.filter((question) => question.group === group);
                return (
                  <section
                    key={group}
                    className="rounded-xl border border-aether-border bg-aether-surface p-4"
                  >
                    <h3 className="mb-2 text-row font-semibold text-aether-text">
                      {label} <span className="text-meta text-aether-text-muted">({questions.length})</span>
                    </h3>
                    <ul className="space-y-3">
                      {questions.map((question, index) => (
                        <li key={index} className="text-body text-aether-text">
                          <p>{question.question}</p>
                          <p className="text-meta text-aether-text-muted">
                            What a strong answer includes: {question.evidence}
                          </p>
                          {(question.claim_ids.length > 0 || question.gap_ids.length > 0) && (
                            <p className="mt-1 flex flex-wrap gap-1">
                              {question.claim_ids.map((id) => (
                                <span
                                  key={id}
                                  className="rounded bg-aether-bg px-1.5 py-0.5 font-mono text-meta text-aether-text-muted"
                                >
                                  {id}
                                </span>
                              ))}
                              {question.gap_ids.map((id) => (
                                <span
                                  key={id}
                                  className="rounded bg-aether-warning/10 px-1.5 py-0.5 font-mono text-meta text-aether-warning"
                                >
                                  gap {id}
                                </span>
                              ))}
                            </p>
                          )}
                        </li>
                      ))}
                    </ul>
                  </section>
                );
              })}
            </div>
          )}

          {prep && tab === 'stories' && (
            <div className="space-y-3">
              {prep.stories.map((story) => (
                <section
                  key={story.theme}
                  className="rounded-xl border border-aether-border bg-aether-surface p-4"
                >
                  <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
                    <h3 className="text-row font-semibold capitalize text-aether-text">
                      {story.theme.replace('-', ' ')}
                    </h3>
                    <span
                      className={`rounded px-2 py-0.5 text-meta ${
                        story.status === 'ready'
                          ? 'bg-aether-success/10 text-aether-success'
                          : 'bg-aether-warning/10 text-aether-warning'
                      }`}
                    >
                      {story.status === 'ready' ? 'Ready' : 'Needs a story'}
                    </span>
                  </div>

                  {story.status === 'ready' ? (
                    <div className="space-y-1 text-body text-aether-text">
                      <p><span className="text-aether-text-muted">Situation:</span> {story.situation}</p>
                      <p><span className="text-aether-text-muted">Task:</span> {story.task}</p>
                      <p><span className="text-aether-text-muted">Action:</span> {story.action}</p>
                      <p><span className="text-aether-text-muted">Result:</span> {story.result}</p>
                      <p className="flex flex-wrap gap-1 pt-1">
                        {story.claim_ids.map((id) => (
                          <span
                            key={id}
                            className="rounded bg-aether-bg px-1.5 py-0.5 font-mono text-meta text-aether-text-muted"
                          >
                            {id}
                          </span>
                        ))}
                        {story.gap_ids.map((id) => (
                          <span
                            key={id}
                            className="rounded bg-aether-primary/10 px-1.5 py-0.5 font-mono text-meta text-aether-primary-ink"
                          >
                            covers {id}
                          </span>
                        ))}
                      </p>
                    </div>
                  ) : (
                    <p className="text-body text-aether-warning">{story.reason}</p>
                  )}
                </section>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}
