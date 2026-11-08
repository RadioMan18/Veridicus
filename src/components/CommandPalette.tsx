import { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { CornerDownLeft } from 'lucide-react';
import ApplyTimeline, { initialStageStates, type StageState } from './ApplyTimeline';
import Markdown from './Markdown';
import MockSessionPanel, {
  weakestTurns,
  type MockDrillResult,
  type MockEvaluation,
  type MockPlan,
  type MockSession,
  type MockTurn,
} from './MockInterview';

export interface PaletteEntry {
  kind: 'info' | 'success' | 'error';
  text: string;
}

interface PaletteCommand {
  id: string;
  description: string;
  hint: string;
  example: string;
  takesArgument: boolean;
}

const COMMANDS: PaletteCommand[] = [
  {
    id: '/apply',
    description: 'Scrape a posting and build a tailored resume and cover letter',
    hint: 'a job posting URL',
    example: '/apply https://boards.greenhouse.io/acme/jobs/123',
    takesArgument: true,
  },
  {
    id: '/prep',
    description: 'Company research briefing for an interview',
    hint: 'a company name',
    example: '/prep NetDocuments',
    takesArgument: true,
  },
  {
    id: '/fit',
    description: 'Role-fit matrix: strengths, gaps and keyword coverage',
    hint: 'no arguments',
    example: '/fit',
    takesArgument: false,
  },
  {
    id: '/questions',
    description: 'Question bank for the latest application',
    hint: 'no arguments',
    example: '/questions',
    takesArgument: false,
  },
  {
    id: '/stories',
    description: 'STAR story bank with per-theme status',
    hint: 'no arguments',
    example: '/stories',
    takesArgument: false,
  },
  {
    id: '/bullets',
    description: 'Strengthen resume bullets and flag missing metrics',
    hint: 'no arguments',
    example: '/bullets',
    takesArgument: false,
  },
  {
    id: '/review',
    description: 'Recruiter-style verdict on the latest application',
    hint: 'no arguments',
    example: '/review',
    takesArgument: false,
  },
  {
    id: '/pack',
    description: 'Assemble the application pack from existing artifacts',
    hint: 'no arguments',
    example: '/pack',
    takesArgument: false,
  },
  {
    id: '/mock',
    description: 'Practice interview: one question at a time, with a debrief',
    hint: 'a role title, optionally --simulation',
    example: '/mock Director of Training --simulation',
    takesArgument: true,
  },
  {
    id: '/resume',
    description: 'Import a resume file and extract claims into the ledger',
    hint: 'an optional file path',
    example: '/resume C:\\Users\\me\\resume.docx',
    takesArgument: false,
  },
  {
    id: '/export',
    description: 'Export the master resume as PDF, DOCX or ODT',
    hint: 'pdf, docx or odt',
    example: '/export docx',
    takesArgument: true,
  },
  {
    id: '/followup',
    description: 'Draft follow-up emails for stale applications',
    hint: 'no arguments',
    example: '/followup',
    takesArgument: false,
  },
  {
    id: '/weekly',
    description: 'Weekly application counts by status',
    hint: 'no arguments',
    example: '/weekly',
    takesArgument: false,
  },
  {
    id: '/honesty-stats',
    description: 'Counts of blocks, overrides and advisory findings from the audit record',
    hint: 'no arguments',
    example: '/honesty-stats',
    takesArgument: false,
  },
  {
    id: '/logs',
    description: 'Daily log and application activity analysis',
    hint: 'no arguments',
    example: '/logs',
    takesArgument: false,
  },
];

const RECENTS_KEY = 'careeros-recent-commands';

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  onExport: (format: 'pdf' | 'docx' | 'odt') => void;
  onShowBriefing?: (content: string, title: string) => void;
  initialCommand?: string;
  history: PaletteEntry[];
  onHistory: (entry: PaletteEntry) => void;
  onApplyComplete?: (result: any) => void;
  onOpenFit?: () => void;
}

export default function CommandPalette({
  isOpen,
  onClose,
  onExport,
  onShowBriefing,
  initialCommand = '',
  history,
  onHistory,
  onApplyComplete,
  onOpenFit,
}: CommandPaletteProps) {
  const [command, setCommand] = useState(initialCommand);
  const [selected, setSelected] = useState(0);
  const [running, setRunning] = useState(false);
  const [applyStates, setApplyStates] = useState<Record<string, StageState>>(initialStageStates);
  const [applyModel, setApplyModel] = useState<string | undefined>(undefined);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [mockSession, setMockSession] = useState<MockSession | null>(null);
  const [mockBusy, setMockBusy] = useState(false);
  const [recents, setRecents] = useState<string[]>([]);
  const outputRef = useRef<HTMLDivElement | null>(null);
  const executingRef = useRef(false);
  const progressWarnedRef = useRef(false);
  const mockPausedHintRef = useRef(false);

  useEffect(() => {
    if (!isOpen) return;
    setCommand(initialCommand);
    setSelected(0);
    try {
      const stored = JSON.parse(localStorage.getItem(RECENTS_KEY) || '[]');
      setRecents(Array.isArray(stored) ? stored : []);
    } catch {
      setRecents([]);
    }
  }, [isOpen, initialCommand]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;
    listen<{ stage: string; status: string; detail?: string | null }>('apply-progress', (event) => {
      const { stage, status, detail } = event.payload;
      setApplyStates((previous) => ({
        ...previous,
        [stage]: { status: status as StageState['status'], detail: detail || undefined },
      }));
    }).then((stop) => {
      if (active) unlisten = stop;
      else stop();
    }).catch(() => {
      // Without the core:event permission the timeline cannot update; say so instead
      // of showing an empty checklist that never advances.
      if (active && !progressWarnedRef.current) {
        progressWarnedRef.current = true;
        onHistory({
          kind: 'error',
          text: 'Live progress is unavailable in this build. The run still finishes; results appear when it completes.',
        });
      }
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    const onEscape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      if (running) {
        onHistory({ kind: 'info', text: 'A run is in progress. Use Cancel run to stop it.' });
        return;
      }
      if (mockSession?.status === 'asking' && !mockPausedHintRef.current) {
        mockPausedHintRef.current = true;
        onHistory({
          kind: 'info',
          text: 'Interview paused. Reopen the palette (Ctrl+K) and it resumes where you stopped.',
        });
      }
      onClose();
    };
    window.addEventListener('keydown', onEscape);
    return () => window.removeEventListener('keydown', onEscape);
  }, [isOpen, running, onClose, onHistory, mockSession]);

  useEffect(() => {
    if (!running) return;
    const startedAt = Date.now() - elapsedMs;
    const timer = setInterval(() => setElapsedMs(Date.now() - startedAt), 250);
    return () => clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [running]);

  useEffect(() => {
    if (outputRef.current) outputRef.current.scrollTop = outputRef.current.scrollHeight;
  }, [history.length, applyStates, mockSession]);

  const ordered = useMemo(() => {
    return [...COMMANDS].sort((a, b) => {
      const aIndex = recents.indexOf(a.id);
      const bIndex = recents.indexOf(b.id);
      if (aIndex === -1 && bIndex === -1) return 0;
      if (aIndex === -1) return 1;
      if (bIndex === -1) return -1;
      return aIndex - bIndex;
    });
  }, [recents]);

  const typed = command.trim();
  const commandWord = (typed.startsWith('/') ? typed.split(/\s+/)[0] : typed).toLowerCase();
  const argument = typed.startsWith('/') ? typed.slice(commandWord.length).trim() : '';
  const mockAnswering = Boolean(mockSession && mockSession.status === 'asking');
  const showList = !mockAnswering && !typed.includes(' ');

  const matches = useMemo(() => {
    if (!commandWord) return ordered;
    const needle = commandWord.replace(/^\//, '');
    return ordered.filter(
      (item) =>
        item.id.slice(1).startsWith(needle) || item.description.toLowerCase().includes(needle),
    );
  }, [ordered, commandWord]);

  const selectedCommand = matches[Math.min(selected, Math.max(matches.length - 1, 0))];

  const remember = (id: string) => {
    const next = [id, ...recents.filter((entry) => entry !== id)].slice(0, 4);
    setRecents(next);
    try {
      localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
    } catch {
      // Recents are a convenience; ignore storage failures.
    }
  };

  const runApply = async (url: string) => {
    setApplyStates(initialStageStates());
    setApplyModel(undefined);
    setElapsedMs(0);
    setRunning(true);
    onHistory({ kind: 'info', text: `Applying: ${url}` });

    try {
      const config: any = await invoke('get_ollama_config').catch(() => null);
      if (config?.deep_model || config?.fast_model) {
        setApplyModel(config.deep_model || config.fast_model);
      }
    } catch {
      // The model name is cosmetic.
    }

    try {
      const result: any = await invoke('execute_apply_workflow', {
        input: {
          jd_url: url,
          employer: null,
          role: null,
          req_id: null,
          defer_commit: true,
        },
      });
      if (result.success) {
        onHistory({
          kind: 'success',
          text: `Materials ready: ${result.employer} — ${result.role}. Nothing committed yet.`,
        });
        onApplyComplete?.(result);
        onClose();
      } else {
        onHistory({
          kind: 'error',
          text: `Apply stopped: ${result.error_message || 'unknown error'}`,
        });
      }
    } catch (error: any) {
      onHistory({ kind: 'error', text: `Apply failed: ${error?.message || String(error)}` });
    } finally {
      setRunning(false);
    }
  };

  const beginMock = async (rawArgument: string) => {
    const simulation = /(^|\s)--(simulation|sim|real|realistic)\b/i.test(rawArgument);
    const role = rawArgument
      .replace(/--\S+/g, ' ')
      .replace(/\s+/g, ' ')
      .trim();
    if (!role) {
      onHistory({
        kind: 'error',
        text: 'Give /mock a role title. Example: /mock Director of Training',
      });
      return;
    }

    setMockBusy(true);
    try {
      const plan: MockPlan = await invoke('plan_mock_interview', {
        role,
        applicationId: null,
        mode: simulation ? 'simulation' : 'coaching',
        questionCount: null,
      });
      mockPausedHintRef.current = false;
      setMockSession({
        plan,
        transcript: [],
        active: null,
        startedAt: Date.now(),
        questionStartedAt: Date.now(),
        status: 'asking',
        debrief: null,
        drill: null,
      });
      onHistory({
        kind: 'success',
        text: `**Interview started — ${role}.** ${plan.contract}`,
      });
    } catch (error: any) {
      onHistory({ kind: 'error', text: error?.message || String(error) });
    } finally {
      setMockBusy(false);
    }
  };

  const finishMock = async (plan: MockPlan, transcript: MockTurn[]) => {
    setMockBusy(true);
    try {
      const report: string = await invoke('finish_mock_interview', {
        role: plan.role,
        mode: plan.mode,
        turns: transcript.map((turn) => ({
          question: turn.question,
          group: turn.group,
          answer: turn.answer,
          followUpAnswer: turn.followUpAnswer,
          skipped: turn.skipped,
        })),
      });
      // The panel is the debrief while the session is open; it moves into the
      // palette output when the session is closed, so it appears once.
      setMockSession((previous) =>
        previous
          ? { ...previous, transcript, active: null, status: 'complete', debrief: report }
          : previous,
      );
    } catch (error: any) {
      setMockSession((previous) =>
        previous ? { ...previous, transcript, active: null, status: 'complete' } : previous,
      );
      onHistory({
        kind: 'error',
        text: `The debrief could not be generated: ${error?.message || String(error)}`,
      });
    } finally {
      setMockBusy(false);
    }
  };

  const answerMock = async (text: string) => {
    if (!mockSession || mockSession.status !== 'asking') return;
    const question = mockSession.plan.questions[mockSession.transcript.length];
    if (!question) return;

    const isFollowUp = Boolean(mockSession.active?.followUp);
    const firstAnswer = mockSession.active?.answer ?? text;
    const elapsedSeconds = Math.max(
      1,
      Math.round((Date.now() - mockSession.questionStartedAt) / 1000),
    );

    setMockBusy(true);
    try {
      const evaluation: MockEvaluation = await invoke('evaluate_mock_answer', {
        role: mockSession.plan.role,
        mode: mockSession.plan.mode,
        question: question.text,
        evidence: question.evidence,
        answer: firstAnswer,
        followUpAnswer: isFollowUp ? text : null,
        elapsedSeconds,
      });

      // One probe per question: the follow-up answer is folded into the same
      // evaluation, so a short answer never turns into an endless loop.
      if (evaluation.follow_up && !isFollowUp) {
        setMockSession({
          ...mockSession,
          active: { answer: firstAnswer, followUp: evaluation.follow_up, ack: evaluation.ack },
        });
        return;
      }

      const turn: MockTurn = {
        question: question.text,
        group: question.group,
        answer: firstAnswer,
        followUpQuestion: mockSession.active?.followUp ?? null,
        followUpAnswer: isFollowUp ? text : null,
        feedback: evaluation.feedback,
        skipped: false,
      };
      const transcript = [...mockSession.transcript, turn];

      if (transcript.length >= mockSession.plan.questions.length) {
        if (mockSession.drill) {
          finishDrillRound(mockSession, transcript);
        } else {
          await finishMock(mockSession.plan, transcript);
        }
      } else {
        setMockSession({
          ...mockSession,
          transcript,
          active: null,
          questionStartedAt: Date.now(),
        });
        if (evaluation.model_note) {
          onHistory({ kind: 'info', text: evaluation.model_note });
        }
      }
    } catch (error: any) {
      onHistory({ kind: 'error', text: error?.message || String(error) });
    } finally {
      setMockBusy(false);
    }
  };

  const skipMock = async () => {
    if (!mockSession || mockSession.status !== 'asking') return;
    const question = mockSession.plan.questions[mockSession.transcript.length];
    if (!question) return;

    const transcript: MockTurn[] = [
      ...mockSession.transcript,
      {
        question: question.text,
        group: question.group,
        answer: '',
        followUpQuestion: null,
        followUpAnswer: null,
        feedback: null,
        skipped: true,
      },
    ];

    if (transcript.length >= mockSession.plan.questions.length) {
      if (mockSession.drill) {
        finishDrillRound(mockSession, transcript);
        return;
      }
      await finishMock(mockSession.plan, transcript);
      return;
    }
    setMockSession({ ...mockSession, transcript, active: null, questionStartedAt: Date.now() });
  };

  const retryMock = () => {
    if (!mockSession || mockSession.status !== 'asking') return;
    if (mockSession.transcript.length === 0) {
      onHistory({ kind: 'info', text: 'Nothing to redo yet.' });
      return;
    }
    setMockSession({
      ...mockSession,
      transcript: mockSession.transcript.slice(0, -1),
      active: null,
      questionStartedAt: Date.now(),
    });
    onHistory({ kind: 'info', text: 'Redoing the previous question.' });
  };

  const endMock = async () => {
    if (!mockSession || mockSession.status !== 'asking') return;
    const transcript = [...mockSession.transcript];

    // An answer in progress is kept: the candidate said it, so it is scored.
    if (mockSession.active) {
      const question = mockSession.plan.questions[transcript.length];
      if (question) {
        transcript.push({
          question: question.text,
          group: question.group,
          answer: mockSession.active.answer,
          followUpQuestion: mockSession.active.followUp,
          followUpAnswer: null,
          feedback: null,
          skipped: false,
        });
      }
    }

    if (transcript.length === 0) {
      onHistory({
        kind: 'info',
        text: 'Interview ended before any answer, so there is nothing to debrief.',
      });
      setMockSession(null);
      return;
    }
    if (mockSession.drill) {
      finishDrillRound(mockSession, transcript);
      return;
    }
    await finishMock(mockSession.plan, transcript);
  };

  /**
   * Close a drill round by comparing each answer with the best score that
   * question has had. A question only leaves the drill by beating itself, so the
   * loop stops when the answers actually improve rather than after a fixed count.
   */
  const finishDrillRound = (session: MockSession, transcript: MockTurn[]) => {
    if (!session.drill) return;
    const baseline = { ...session.drill.baseline };
    const results: MockDrillResult[] = transcript.map((turn) => {
      const previous = baseline[turn.question] ?? 0;
      const score = turn.feedback?.overall ?? 0;
      return { question: turn.question, previous, score, improved: score > previous };
    });
    for (const result of results) {
      baseline[result.question] = Math.max(baseline[result.question] ?? 0, result.score);
    }

    setMockSession({
      ...session,
      transcript,
      active: null,
      status: 'complete',
      debrief: null,
      drill: { ...session.drill, baseline, results },
    });
    const improved = results.filter((result) => result.improved).length;
    onHistory({
      kind: improved === results.length ? 'success' : 'info',
      text: `Drill round ${session.drill.round}: ${improved} of ${results.length} answer(s) improved.`,
    });
  };

  const startDrill = async (
    questions: string[],
    baseline: Record<string, number>,
    round: number,
    role: string,
  ) => {
    if (questions.length === 0) return;
    setMockBusy(true);
    try {
      const plan: MockPlan = await invoke('plan_mock_drill', {
        role,
        applicationId: null,
        questions,
      });
      mockPausedHintRef.current = false;
      setMockSession({
        plan,
        transcript: [],
        active: null,
        startedAt: Date.now(),
        questionStartedAt: Date.now(),
        status: 'asking',
        debrief: null,
        drill: { baseline, round, results: [], role },
      });
      onHistory({
        kind: 'info',
        text: `Drill round ${round} — ${plan.questions.length} question(s), weakest first.`,
      });
    } catch (error: any) {
      onHistory({ kind: 'error', text: error?.message || String(error) });
    } finally {
      setMockBusy(false);
    }
  };

  const drillSession = () => {
    if (!mockSession) return;
    const pending = mockSession.drill
      ? mockSession.drill.results
          .filter((result) => !result.improved)
          .map((result) => result.question)
      : weakestTurns(mockSession.transcript).map((turn) => turn.question);
    if (pending.length === 0) {
      onHistory({ kind: 'info', text: 'There is nothing left to drill.' });
      return;
    }

    // The next round is measured against the best score so far, including the
    // answers just given, so it cannot pass by getting worse.
    const baseline: Record<string, number> = { ...(mockSession.drill?.baseline ?? {}) };
    for (const turn of mockSession.transcript) {
      if (!turn.feedback) continue;
      baseline[turn.question] = Math.max(baseline[turn.question] ?? 0, turn.feedback.overall);
    }

    void startDrill(
      pending,
      baseline,
      (mockSession.drill?.round ?? 0) + 1,
      mockSession.drill?.role ?? mockSession.plan.role,
    );
  };

  const execute = async () => {
    if (executingRef.current || running || mockBusy) return;

    // During a session the input is an answer box. Only the session commands run
    // as commands; everything else is the candidate's answer.
    const typedText = command.trim();
    if (mockSession && mockSession.status === 'asking') {
      if (!typedText) return;
      if (executingRef.current) return;
      executingRef.current = true;
      try {
        if (typedText.startsWith('/')) {
          const sessionCommand = typedText.split(/\s+/)[0].toLowerCase();
          if (['/end', '/exit', '/stop'].includes(sessionCommand)) {
            await endMock();
          } else if (sessionCommand === '/skip') {
            await skipMock();
          } else if (sessionCommand === '/retry') {
            retryMock();
          } else {
            onHistory({
              kind: 'error',
              text: `During an interview only /skip, /retry and /end run. "${typedText}" was not sent as an answer.`,
            });
          }
        } else {
          await answerMock(typedText);
        }
      } finally {
        executingRef.current = false;
        setCommand('');
      }
      return;
    }

    if (mockSession && mockSession.status === 'complete' && typedText && !typedText.startsWith('/')) {
      onHistory({ kind: 'info', text: 'This interview is finished. Start another with /mock <role>.' });
      return;
    }

    const known = COMMANDS.find((item) => item.id === commandWord);
    if (!known) {
      const needle = commandWord.replace(/^\//, '').slice(0, 2);
      const guess = COMMANDS.find((item) => item.id.slice(1).startsWith(needle));
      onHistory({
        kind: 'error',
        text: guess
          ? `Unknown command. Did you mean ${guess.id}? Type / to see all eight.`
          : 'Unknown command. Type / to see all eight commands.',
      });
      return;
    }

    if (known.takesArgument && !argument) {
      onHistory({
        kind: 'error',
        text: `${known.id} needs ${known.hint}. Example: ${known.example}`,
      });
      return;
    }

    remember(known.id);
    executingRef.current = true;

    try {
      if (known.id === '/apply') {
        if (!/^https?:\/\//i.test(argument)) {
          onHistory({
            kind: 'error',
            text: 'That is not a URL. Paste the full https:// address of the job posting.',
          });
          return;
        }
        await runApply(argument);
        return;
      }

      if (known.id === '/mock') {
        // Clear the input: the next Enter is the first answer, not the command.
        setCommand('');
        await beginMock(argument);
        return;
      }

      setRunning(true);

      if (known.id === '/prep') {
        const briefing: string = await invoke('run_company_research', { company: argument });
        onShowBriefing?.(briefing, argument);
        onHistory({ kind: 'success', text: `Briefing ready for ${argument}.` });
        onClose();
        return;
      }

      if (known.id === '/fit') {
        onOpenFit?.();
        onClose();
        return;
      }

      if (known.id === '/questions' || known.id === '/stories') {
        const records: any[] = await invoke('get_application_records');
        const latest = Array.isArray(records) && records.length > 0 ? records[0] : null;
        if (!latest) {
          onHistory({ kind: 'error', text: 'No applications yet. Run /apply first.' });
          return;
        }
        await invoke('generate_interview_prep', { applicationId: latest.id });
        const markdown: string = await invoke('get_interview_prep_markdown', {
          applicationId: latest.id,
        });
        const [questions, stories] = markdown.split('## STAR bank');
        onHistory({
          kind: 'success',
          text:
            known.id === '/questions'
              ? questions.trim()
              : `## STAR bank\n${(stories || '').trim()}`,
        });
        return;
      }

      if (known.id === '/bullets') {
        const resume: string = await invoke('get_master_resume');
        const bullets = resume
          .split('\n')
          .map((line) => line.trim())
          .filter((line) => line.startsWith('- '))
          .map((line) => line.slice(2).trim())
          .filter((line) => line.length > 20)
          .slice(0, 12);
        if (bullets.length === 0) {
          onHistory({ kind: 'error', text: 'No resume bullets found. Import a resume first.' });
          return;
        }
        const report: any = await invoke('review_bullets', {
          applicationId: null,
          bullets,
          jdText: null,
        });
        const lines: string[] = (report.bullets || []).map((bullet: any) => {
          const head = `**${bullet.status === 'rewritten' ? 'Rewritten' : bullet.status === 'blocked' ? 'Blocked' : 'Needs your input'}** — ${bullet.original}`;
          const body = bullet.rewrite ? `\n→ ${bullet.rewrite}` : `\n→ ${bullet.problem || 'no rewrite'}`;
          const markers = bullet.markers?.length ? `\nMarkers: ${bullet.markers.join(', ')}` : '';
          return `${head}${body}${markers}`;
        });
        onHistory({ kind: 'success', text: `## Bullet review\n\n${lines.join('\n\n')}` });
        return;
      }

      if (known.id === '/review') {
        const review: any = await invoke('generate_recruiter_review', { applicationId: null });
        onHistory({ kind: 'success', text: review.markdown });
        return;
      }

      if (known.id === '/pack') {
        const pack: any = await invoke('generate_application_pack', { applicationId: null });
        onHistory({ kind: 'success', text: pack.markdown });
        return;
      }

      if (known.id === '/followup') {
        const drafts: string = await invoke('draft_followups');
        onHistory({ kind: 'success', text: drafts });
        return;
      }

      if (known.id === '/weekly') {
        const report: string = await invoke('generate_weekly_report');
        onHistory({ kind: 'success', text: report });
        return;
      }

      if (known.id === '/logs') {
        const analysis: string = await invoke('analyze_logs');
        onHistory({ kind: 'success', text: analysis });
        return;
      }

      if (known.id === '/resume') {
        const result: any = await invoke('open_resume_file', { filePath: argument || null });
        onHistory(
          result.success
            ? { kind: 'success', text: `Resume imported. ${result.claims_imported} claims need confirmation in Setup before they can be used.` }
            : { kind: 'error', text: result.error_message || 'Resume import failed.' },
        );
        return;
      }

      if (known.id === '/honesty-stats') {
        const stats: any = await invoke('get_honesty_stats');
        const blocks = Object.entries(stats.blocks_by_code || {})
          .map(([code, count]) => `${code}: ${count}`)
          .join(', ');
        const review = stats.next_review_due
          ? `Next review due ${String(stats.next_review_due).slice(0, 10)}${stats.review_overdue ? ' (overdue)' : ''}.`
          : 'No review recorded.';
        const overrideRate = Math.round((stats.override_rate || 0) * 100);
        onHistory({
          kind: 'success',
          text: `Honesty record across ${stats.applications} application(s): ${stats.generated} generated, ${stats.approved} approved, ${stats.overrides} override(s) (${overrideRate}%), ${stats.blocked_findings} blocking finding(s), ${stats.advisory_findings} advisory, ${stats.false_block_reports || 0} false-block report(s).${blocks ? ` Blocks by code: ${blocks}.` : ''} ${review}`,
        });
        return;
      }

      if (known.id === '/export') {
        const format = argument.toLowerCase();
        if (format !== 'pdf' && format !== 'docx' && format !== 'odt') {
          onHistory({ kind: 'error', text: 'Use /export pdf, /export docx or /export odt.' });
          return;
        }
        onExport(format);
        onClose();
      }
    } catch (error: any) {
      onHistory({ kind: 'error', text: error?.message || String(error) });
    } finally {
      executingRef.current = false;
      setRunning(false);
    }
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      if (running) {
        onHistory({ kind: 'info', text: 'A run is in progress. Use Cancel run to stop it.' });
        return;
      }
      if (mockAnswering && !mockPausedHintRef.current) {
        mockPausedHintRef.current = true;
        onHistory({
          kind: 'info',
          text: 'Interview paused. Reopen the palette (Ctrl+K) and it resumes where you stopped.',
        });
      }
      onClose();
      return;
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setSelected((index) => Math.min(index + 1, Math.max(matches.length - 1, 0)));
      return;
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault();
      setSelected((index) => Math.max(index - 1, 0));
      return;
    }

    if (event.key === 'Tab') {
      event.preventDefault();
      if (mockAnswering) return;
      if (selectedCommand) {
        setCommand(`${selectedCommand.id} `);
        setSelected(0);
      }
      return;
    }

    if (event.key === 'Enter') {
      event.preventDefault();
      // During a session Enter submits the answer. It must not be read as
      // "complete the highlighted command" — that hijacked every typed answer.
      if (mockAnswering) {
        void execute();
        return;
      }
      if (showList && selectedCommand && selectedCommand.takesArgument && !argument) {
        setCommand(`${selectedCommand.id} `);
        setSelected(0);
        return;
      }
      void execute();
    }
  };

  if (!isOpen) return null;

  return (
    <div
      className="dialog-backdrop fixed inset-0 z-50 flex items-start justify-center bg-black/60 p-4 pt-[10vh] backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
    >
      <div className="flex max-h-[80vh] w-full max-w-3xl flex-col overflow-hidden rounded-xl border border-aether-border bg-aether-surface shadow-2xl">
        <div className="flex items-center gap-2 border-b border-aether-border p-4">
          <span className="font-mono font-bold text-aether-primary-ink">&gt;_</span>
          <input
            type="text"
            value={command}
            onChange={(event) => {
              setCommand(event.target.value);
              setSelected(0);
            }}
            onKeyDown={handleKeyDown}
            placeholder={
              mockAnswering
                ? mockSession?.active?.followUp
                  ? 'Answer the follow-up, then press Enter'
                  : `Your answer for ${mockSession?.plan.role} — Enter sends, /skip passes`
                : mockSession?.status === 'complete'
                  ? 'Session finished. Start another with /mock <role>'
                  : 'Type / to see every command, e.g. /apply https://company.com/job'
            }
            className="w-full bg-transparent font-mono text-aether-text outline-none placeholder:text-aether-text-muted/70"
            autoFocus
            aria-label="Command input"
          />
        </div>

        {showList && matches.length > 0 && (
          <ul className="max-h-64 overflow-y-auto border-b border-aether-border" role="listbox">
            {matches.map((item, index) => (
              <li key={item.id}>
                <button
                  onMouseEnter={() => setSelected(index)}
                  onClick={() => {
                    setCommand(`${item.id} `);
                    setSelected(0);
                  }}
                  className={`flex w-full items-start gap-3 px-4 py-2 text-left transition ${
                    index === selected ? 'bg-aether-primary/15' : 'hover:bg-aether-primary/10'
                  }`}
                >
                  <span className="w-24 shrink-0 font-mono text-meta text-aether-primary-ink">
                    {item.id}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block text-sm text-aether-text">{item.description}</span>
                    <span className="block truncate text-xs text-aether-text-muted">
                      {item.takesArgument ? `takes ${item.hint}` : item.hint} · {item.example}
                    </span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}

        {showList && matches.length === 0 && (
          <div className="border-b border-aether-border px-4 py-3 text-sm text-aether-text-muted">
            No command matches “{commandWord}”. Type / to list all eight.
          </div>
        )}

        {mockAnswering ? (
          <div className="border-b border-aether-border px-4 py-2 font-mono text-xs text-aether-text-muted">
            enter answers · /skip passes · /retry redoes · /end debriefs
          </div>
        ) : !showList ? (
          <div className="border-b border-aether-border px-4 py-2 font-mono text-xs text-aether-text-muted">
            {selectedCommand?.id === commandWord || !selectedCommand
              ? COMMANDS.find((item) => item.id === commandWord)?.hint
              : `tab completes ${selectedCommand?.id}`}
            {' · '}
            enter runs
          </div>
        ) : null}

        <div
          ref={outputRef}
          className="flex-1 overflow-y-auto whitespace-pre-wrap bg-aether-bg p-4 text-sm text-aether-text"
        >
          {running && (
            <div className="mb-3">
              <ApplyTimeline
                states={applyStates}
                elapsedMs={elapsedMs}
                model={applyModel}
                running={running}
                onCancel={() => {
                  void invoke('cancel_apply');
                  onHistory({ kind: 'info', text: 'Cancellation requested…' });
                }}
              />
            </div>
          )}

          {history.length === 0 && !running && (
            <p className="text-aether-text-muted">
              Ready. Type / to browse commands, Tab to complete, Enter to run.
            </p>
          )}

          {history.map((entry, index) =>
            entry.kind === 'error' ? (
              <p
                key={`${index}-error`}
                className="mt-2 whitespace-pre-wrap font-mono text-xs text-aether-error"
              >
                {entry.text}
              </p>
            ) : (
              <div key={`${index}-${entry.kind}`} className="mt-2">
                <Markdown content={entry.text} />
              </div>
            ),
          )}

          {mockSession && (
            <MockSessionPanel
              session={mockSession}
              busy={mockBusy}
              onEnd={() => {
                void endMock();
              }}
              onRetry={retryMock}
              onRestart={() => {
                void beginMock(
                  `${mockSession.plan.role}${
                    mockSession.plan.mode === 'simulation' ? ' --simulation' : ''
                  }`,
                );
              }}
              onDrill={drillSession}
              onDone={() => {
                if (mockSession.drill && mockSession.drill.results.length > 0) {
                  const improved = mockSession.drill.results.filter(
                    (result) => result.improved,
                  ).length;
                  onHistory({
                    kind: 'success',
                    text: `**Drill round ${mockSession.drill.round}** — ${improved} of ${mockSession.drill.results.length} answer(s) improved.`,
                  });
                } else if (mockSession.debrief) {
                  onHistory({ kind: 'success', text: mockSession.debrief });
                }
                setMockSession(null);
              }}
            />
          )}
        </div>

        <div className="flex flex-wrap justify-between gap-2 border-t border-aether-border bg-aether-surface p-2 text-xs text-aether-text-muted">
          <span className="flex items-center gap-1">
            <CornerDownLeft size={12} />
            {mockAnswering ? 'enter answer · /skip · /retry · /end' : 'run · tab complete · ↑↓ browse'}
          </span>
          <span>Esc closes · Ctrl+K toggles</span>
        </div>
      </div>
    </div>
  );
}
