# /mock interview UI plan

Scope: the mock-interview session (`/mock`) in the command palette. Companion work on
question grounding is already in `src-tauri/src/interview_prep.rs`; this plan covers
what the session shows and how it feels. Interviewer personas (section 5) are
implemented with this plan; the rest are ordered proposals.

## 1. What /mock is today

| Piece | Location |
| --- | --- |
| Session state, commands, drill loop | `src/components/CommandPalette.tsx` (`beginMock`, `answerMock`, `skipMock`, `endMock`, `drillSession`) |
| Session panel, feedback card | `src/components/MockInterview.tsx` (`MockSessionPanel`, `FeedbackCard`, `weakestTurns`) |
| Planning, scoring, debrief | `src-tauri/src/mock_interview.rs` |
| Grounded question bank | `src-tauri/src/interview_prep.rs` (`assemble_questions`) |

Current flow: `/mock <role> [--simulation]` plans immediately, the panel shows one
question at a time above its transcript, each answer is scored locally and optionally
coached by the model, and the run ends in a markdown debrief that is handed to the
command history.

Strengths worth keeping: questions are grounded in the ledger, scoring is
deterministic without the model, the drill loop already compares against a best score,
and a follow-up probe exists for thin answers.

## 2. Benchmark

| Product | What it gets right | Source |
| --- | --- | --- |
| Google Interview Warmup | One question per screen; insights instead of grades (talking points, job-related terms, most-used words); Redo/Edit before advancing; explicitly judgement-free | [blog.google](https://blog.google/company-news/outreach-and-initiatives/grow-with-google/interview-warmup/), [grow.google](https://grow.google/certificates/interview-warmup), [walkthrough](https://www.makeuseof.com/how-to-use-googles-interview-warmup) |
| Yoodli | Preflight setup (role, company, interviewer demeanour, question bank) before the first question; spoken follow-ups; report on pacing, fillers, clarity, structure; progress as first attempt vs best score | [practice setup](https://support.yoodli.ai/en/articles/9550465-practice-with-yoodli), [feedback](https://yoodli.ai/platform/ai-feedback), [progress](https://support.yoodli.ai/en/articles/12611007-admin-analytics-hub) |
| Big Interview | Three explicit practice modes; re-record before saving; per-answer scorecard; saved recordings accumulate | [practice tools](https://iup.biginterview.com/), [VideoAI feedback](https://support.biginterview.com/en/article/videoai-feedback-9vhrsb) |
| Exponent / Pramp | Visible time tracking during the answer; never re-ask the same question across sessions | [Exponent Practice](https://www.tryexponent.com/blog/introducing-exponent-practice), [Pramp FAQ](https://www.pramp.com/faq) |
| Instant Interview, Bossed | Lead with pacing (~1:28 per answer) and a timer; give one next step instead of a wall of scores | [Instant Interview](https://instantinterview.app/), [Bossed](https://bossed.ai/ai-interview-practice) |

## 3. Recommendations, in priority order

### R1. Per-question timer with a soft target
Show a live clock next to the question (`1:12`), a soft target ("90–120s is typical,
3 minutes is long"), and add pacing to the coach card using the `elapsedSeconds`
already sent to `evaluate_mock_answer`.

- Where: `MockSessionPanel` (timer effect), `AnswerMetrics` line in `FeedbackCard`.
- Done when: the clock survives a follow-up (it should keep running or restart
  explicitly — pick restart on follow-up and say so), and a 4-minute answer is
  reported as long without changing the score.

### R2. Keep the current question and composer in view
The transcript grows above them; after two coach cards the question being answered is
off-screen. Pin the current question and composer to the bottom of the panel (or split
the panel: transcript scrolls, current block fixed) and collapse older coach cards to a
one-line `Question 2 · 71/100` chip that expands on click.

- Where: `MockSessionPanel` layout.
- Done when: with six answered questions, the current question and textarea are visible
  without scrolling.

### R3. STAR coverage as a checklist
Render `metrics.star` as four ticks with the missing part named ("no result — say what
changed and how you measured it"). Optionally highlight the sentence that carried the
result, and list posting terms from the question's evidence that the answer never used.

- Where: `FeedbackCard`, which already receives `metrics` and `priority`.
- Done when: a missing result is visible without reading the prose.

### R4. Redo keeps both attempts
`/retry` currently slices the last turn off, so improvement is invisible. Keep the
previous attempt per question and show `68 → 81`, the pattern the drill already
implements with its baseline.

- Where: `retryMock` in `CommandPalette.tsx`, turn type in `MockInterview.tsx`.
- Done when: retrying a question shows both scores and which fixes were addressed.

### R5. Preflight setup instead of hidden flags
`/mock <role> --simulation` hides mode and length behind syntax. A setup card (role
prefilled from the latest application, coaching/simulation toggle, 3/6/12 questions,
focus area, interviewer persona) is what Yoodli and Big Interview do.

- Where: `MockSetupPanel` (added with personas), `beginMock` in `CommandPalette.tsx`.
- Status: persona selection is implemented; mode and question count still arrive as
  flags and can move into the same card next.

### R6. Session history
Nothing survives "Close the session" except a trimmed history entry. Persist sessions
as JSON next to `interview-prep.json` and show a compact "last 5 sessions" strip (date,
role, average score, weakest dimension), building on the drill baseline.

- Where: `mock_interview.rs` (write/read), `PrepView` or the session panel footer.

### R7. Durable debrief with copy and save
The debrief is markdown pushed into command history, which is capped at 60 entries.
Add "Copy debrief" (the `Toaster` helper already exposes `copyText`) and a save into
the application's document folder.

### R8. Soften the grade in coaching mode
Warmup's framing ("no grades, just insights") is why people practise there. In coaching
mode lead with the fix and the STAR ticks and keep the numeric score for simulation
mode, where a verdict is the point.

### R9. Rotate the question set
`/mock` deterministically returns the same six questions from a stable bank. Rotate or
randomise within the grounded bank so a second run is not a memory test, and record
which questions were asked to avoid immediate repeats.

## 4. Non-goals

- Video recording, eye-contact tracking and avatar interviewers (Big Interview,
  Yoodli): needs a camera pipeline and conflicts with the local-first posture.
- Voice dictation: WebView2 speech recognition generally needs an online recogniser;
  only revisit with an offline model.

## 5. Interviewer personas (implemented)

Purpose: let the candidate rehearse the *emotional* shape of an interview, not just the
questions. Four voices, selected before the first question.

| Id | Label | Voice |
| --- | --- | --- |
| `supportive` | Supportive | Warm, leads with what worked, gentle but concrete criticism |
| `professional` | Professional | Neutral, specific, fair (default) |
| `skeptical` | Skeptical | Does not accept a claim without evidence; names the doubt and what would settle it |
| `impatient` | Impatient | Terse, time-pressed, cuts to the single thing that wasted time; never insults |

Aliases accepted on the command line: `easy`/`warm`/`friendly` → supportive,
`hard`/`tough`/`probing` → skeptical, `frustrated`/`strict`/`stress` → impatient,
anything else → professional.

### Invariants

- The persona changes **tone only**. Measured delivery, dimension scores, overall
  score and the deterministic fallback are identical across personas, so a harsh voice
  cannot manufacture a worse score.
- The persona never invents facts and never relaxes the "no invented achievements"
  instruction given to the model.
- A session keeps its persona for the whole run, including its drill rounds.

### Backend

- `MockPlan.persona` carries the normalised id to the panel.
- `plan_mock_interview(role, application_id, mode, persona, question_count)` and
  `plan_mock_drill(role, application_id, questions, persona)` accept it.
- `evaluate_mock_answer(..., persona)` and `finish_mock_interview(..., persona)` use it
  for the interviewer reaction (`ack`), the follow-up lead-in, the coaching system
  prompt and the debrief voice.
- Helpers: `normalize_persona`, `persona_ack`, `persona_follow_up_ack`,
  `persona_system`.

### Frontend

- `MockSetupPanel` lists the four personas with a one-line blurb each, remembers the
  choice for the session, and starts the plan.
- `/mock <role>` now opens the setup card; `/mock <role> --persona skeptical` (or
  `--voice`) skips it.
- The session header shows the persona next to the Coaching/Simulation badge.
- "Practice again" reuses the same persona; the setup card is only shown on request.

### Verification

- `cargo test --bins`: persona aliases normalise, every persona has a distinct ack and
  prompt, and the fallback score is unchanged by persona.
- `npx tsc --noEmit` and `npm run build` after the UI change.
- Browser check of the setup card and persona badge (harness with a stubbed Tauri
  bridge), then a real session with Ollama.

## 6. Delivery order

1. Persona selection (this change).
2. R1 timer, R3 STAR checklist — small, self-contained, high value.
3. R2 pinned question/composer, R4 redo comparison — layout and state work.
4. R5 complete preflight (mode, length, focus), R9 rotation.
5. R6 history, R7 debrief copy/save, R8 coaching-mode framing.

## 7. Risks

- Layout work (R2) touches the same component that the composer fix just changed;
  re-run the harness session afterwards.
- Persisting sessions (R6) must not write outside the application folder or duplicate
  the prep JSON.
- Persona prompts can bias scoring through the model's `overall`; the deterministic
  dimensions are the backstop and must stay untouched.
