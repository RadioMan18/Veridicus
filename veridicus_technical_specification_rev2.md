# Technical Specification: Veridicus Desktop (Rev2)

**System Architecture & Technical Design for a Standalone Desktop Job Search OS powered by Tauri, Local Ollama LLMs, SQLCipher Encryption, and an Honesty Guardrail Verification Engine**

> **Revision Notes (Rev2):** Folds in the *Honesty guardrails for a resume tailor* requirements: SQLCipher-encrypted Claims Ledger (SQLite), four-entity rules schema, 7-stage guardrailed verification pipeline, post-hoc fact-checking with never-claim scanning, hardened metric extraction, mdast-style AST diffing, automated OCR visual review gate, employer-consistency warnings, exact-wording auto-promotion, early `PENDING_REVIEW` application records, CSV↔DB sync, bundled sidecar binaries, and restoration of the workspace bootstrap, Git engine, skill execution engine, command palette, Ollama model manager, and the full slash-command set (`/apply`, `/prep`, `/mock`, `/followup`, `/weekly`). Also adds pre-generation clarification, `pdftotext -bbox` layout analysis with bundled Tesseract, prebuilt sidecar binaries (no MSVC compilation), presentation-rule editor, queryable `daily-log.md` log analysis, and full-text employer-consistency similarity. The `forms` rule scope was dropped.

---

## 1. Executive Overview & Honesty Principles

**Veridicus** is a privacy-first, desktop-native application designed to transform job search workflows into structured, file-based projects with an uncompromising **Honesty Verification Pipeline**.

Built with **Tauri v2** (Rust backend + React/TypeScript frontend) and integrated directly with a local **Ollama** runtime, Veridicus eliminates reliance on third-party SaaS tools or cloud AI API keys.

### Core Honesty Principle
> *"The model proposes, the record decides. Every claim traces to a confirmed fact; anything unsupported is flagged, never smoothed over."*

Veridicus guarantees **Zero-Hallucination Resume Generation** by introducing a local SQLCipher-encrypted Claims Ledger database, strict presentation rules, mandatory gap-naming logic, employer-consistency warnings, an automated pre-render Fact-Check Pass (never-claim scanning, scope verification, hardened metric extraction, and mdast-style AST diffing), an OCR-driven Visual Review Gate, and human-in-the-loop (HITL) approval gates before any document is finalized.

```
+-----------------------------------------------------------------------------------+
|                                 Veridicus Desktop                                  |
|                                                                                   |
|  +-----------------------------------------------------------------------------+  |
|  |                           React / TS UI Frontend                            |  |
|  |  +--------------------+  +---------------------+  +----------------------+  |  |
|  |  | Claims Ledger &    |  | Command Palette &   |  | Fact-Check, Gap &    |  |  |
|  |  | Rule Manager       |  | Ollama Model Manager|  | Visual Review Gates  |  |  |
|  |  +--------------------+  +---------------------+  +----------------------+  |  |
|  +-------------------------------------+---------------------------------------+  |
|                                        | Tauri IPC Communication                  |
|  +-------------------------------------v---------------------------------------+  |
|  |                           Tauri Rust Core Backend                           |  |
|  |  +--------------------+  +---------------------+  +----------------------+  |  |
|  |  | SQLCipher Ledger   |  | Skill Execution &   |  | Fact-Check Gate &    |  |  |
|  |  | Engine (Claims,    |  | Guardrailed         |  | Typst / pdftoppm /   |  |  |
|  |  | Rules, Gaps, Apps) |  | Generation          |  | OCR Render Pipeline  |  |  |
|  |  +--------------------+  +---------------------+  +----------------------+  |  |
|  +-------------------------------------+---------------------------------------+  |
+----------------------------------------|------------------------------------------+
                                         | HTTP / REST (http://localhost:11434)
                                         v
+-----------------------------------------------------------------------------------+
|                                 Ollama Service                                    |
|  +---------------------------+  +----------------------------------------------+  |
|  | Model Registry            |  | Guardrailed Inference Engine                 |  |
|  | (/api/tags, /api/pull)    |  | (Ledger-Bounded Prompts + Post-Hoc Checks)   |  |
|  +---------------------------+  +----------------------------------------------+  |
+-----------------------------------------------------------------------------------+
```

---

## 2. Tech Stack & Key Specifications

| Component | Technology | Description & Implementation |
| :--- | :--- | :--- |
| **Framework** | **Tauri v2** | Lightweight, cross-platform desktop shell (Rust backend + React/TS frontend). |
| **Frontend UI** | **React + Tailwind CSS** | Workspace dashboard, Claims Ledger editor, Command Palette (`Cmd+K` / `Ctrl+K`), Ollama Model Manager, Fact-Check & Gap Verification view, Visual Review Gate. Styling via Tailwind, Lucide icons, Radix UI / Shadcn primitives. |
| **Database Engine** | **SQLCipher (`rusqlite` + `bundled-sqlcipher`)** | Encrypted local relational database (`veridicus.db`) storing the Claims Ledger, Rules Engine, Gaps, and Application Records. Key generated app-side, held in Windows PasswordVault, wrapped with a user recovery code; wrong key fails closed. |
| **Local LLM Engine** | **Ollama REST API** | Local inference via `http://localhost:11434`; ledger-bounded prompts plus mandatory post-hoc fact-checking to prevent hallucination. |
| **Job Scraper** | **Rust (`reqwest` + `scraper` + `htmd`)** | Built-in HTTP client fetching public job pages, stripping boilerplate, and converting main job text to dated Markdown files. |
| **Document Compiler** | **Typst CLI (bundled) + `pdftoppm`** | Version-pinned sidecar binaries compile tailored Markdown resumes into ATS-friendly PDFs and rasterize every page to PNG for visual review. No PATH dependency. |
| **Visual QA** | **OCR + Layout Analysis (bundled)** | Automated inspection of rendered page images: overflow, orphan headings, broken line wraps, placeholder text, wrong role/company names. Findings flag for HITL approval. |
| **Structural Diff** | **Markdown AST (mdast-style, via `comrak`)** | Drafts parsed to a Markdown AST and structurally diffed against the master resume to list every change from the base. |
| **Storage & Audit** | **Local FS + Git (`git2`)** | File workspace at `~/Documents/Veridicus/` with automated Git commits; `applications.csv` synced with the SQLite ledger. |

---

## 3. Database Schema & Data Models (SQLCipher-Encrypted SQLite)

The core database lives at `~/Documents/Veridicus/veridicus.db`, encrypted with SQLCipher. The 32-byte database key is generated on first launch, stored in Windows PasswordVault, wrapped with a user recovery code (PBKDF2 100k iterations + AES-256-GCM), and applied via `sqlite3_key()` **before any schema operation**. A wrong or missing key fails closed. Four primary entity tables carry the whole system:

```sql
-- 1. Candidate Claims Ledger
CREATE TABLE claims (
    id TEXT PRIMARY KEY,                     -- UUID v4
    statement TEXT NOT NULL,
    evidence_source TEXT NOT NULL,
    confirmed_wording TEXT NOT NULL,
    scope TEXT NOT NULL,                     -- e.g., 'internal team', 'personal project', 'company-wide'
    category TEXT CHECK(category IN ('metric', 'tool', 'cert', 'tenure', 'title', 'project')),
    status TEXT CHECK(status IN ('confirmed', 'needs-confirmation', 'rejected')),
    last_confirmed_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 2. Rules Engine (never_claim / ask_first / exact_wording / presentation)
CREATE TABLE rules (
    id TEXT PRIMARY KEY,                     -- UUID v4
    kind TEXT CHECK(kind IN ('never_claim', 'ask_first', 'exact_wording', 'presentation')),
    subject TEXT NOT NULL,
    rule_text TEXT NOT NULL,
    applies_to TEXT CHECK(applies_to IN ('resume', 'cover_letter', 'all')),
    source TEXT NOT NULL,                    -- e.g., 'user-stated 2026-10-30'
    active BOOLEAN DEFAULT 1
);

-- 3. Application Gaps Tracking
CREATE TABLE gaps (
    id TEXT PRIMARY KEY,                     -- UUID v4
    application_id TEXT NOT NULL,
    requirement TEXT NOT NULL,
    requirement_tier TEXT CHECK(requirement_tier IN ('must_have', 'nice_to_have', 'gate')),
    matched_claim_id TEXT REFERENCES claims(id),
    adjacent_strength TEXT NOT NULL,
    letter_line TEXT,
    status TEXT CHECK(status IN ('open', 'named_in_letter', 'accepted_by_user')),
    FOREIGN KEY(application_id) REFERENCES application_records(id)
);

-- 4. Application Ledger & History
CREATE TABLE application_records (
    id TEXT PRIMARY KEY,                     -- UUID v4
    employer TEXT NOT NULL,
    role TEXT NOT NULL,
    req_id TEXT,
    date TEXT NOT NULL,
    base_resume_version TEXT NOT NULL,
    materials_version TEXT NOT NULL,
    claims_used TEXT NOT NULL,               -- JSON array of claim IDs
    approved_wording TEXT NOT NULL,          -- JSON snapshot of the exact approved wording used
    gaps_named TEXT NOT NULL,                -- JSON array of gap IDs
    salary_entered TEXT,
    status TEXT CHECK(status IN ('PENDING_REVIEW', 'SUBMITTED', 'INTERVIEWING', 'REJECTED', 'OFFER')),
    notes TEXT
);
```

### 3.1 Storage & Sync Notes
* **CSV sync:** `applications.csv` is a human-readable mirror of `application_records`. SQLite is the source of truth; every ledger write updates the database first, then writes the corresponding CSV row, so the two never diverge. A reconcile command repairs drift if the files are edited externally. CSV columns mirror the table plus `job_url` and `folder_path`: `id, date, employer, role, status, job_url, folder_path, req_id, base_resume_version, materials_version, claims_used, approved_wording, gaps_named, salary_entered, notes`.
* **IDs:** all primary keys are UUID v4 strings generated in Rust.
* **Sensitivity:** the claims ledger is the most career-sensitive record a user owns — what they have done, what they have not, and what they are sensitive about. It never leaves the machine, the database file is encrypted at rest, and the key never appears in plaintext config, registry, or logs.

---

## 4. Honesty Guardrail Architecture & Verification Pipeline

```
[ Raw Job URL ]
       │
       ▼
[ 0. Application Record Created ] ──► application_records row, status PENDING_REVIEW
       │                                (gaps FK target exists before analysis)
       ▼
[ 1. HTTP Scraper ] ──► Saves dated job-description.md beside the record
       │
       ▼
[ 2. JD Analyzer & Gap Mapper ] ──► Splits posting into must-haves / nice-to-haves / gate items
       │                            Maps each against the SQLite Claims Ledger
       │                            Outputs: Supported Claims, Gaps, Ask-First items,
       │                            Ambiguous requirements needing clarification
       ▼
[ 2.5 Pre-Generation Clarification ] ──► User resolves ambiguous JD requirements
       │                                 and ledger gaps before any generation
       ▼
[ 3. Guardrailed Generation ] ──► Prompt = SYSTEM.md + confirmed claims + active rules
       │                          needs-confirmation claims excluded
       │                          ask_first items pause for user modal first
       ▼
[ 4. Rust Fact-Check Pass ] ──► Hardened metric regex + qualifier preservation
       │                        never_claim subject scan, scope-lock verification
       │                        mdast-style AST diff vs master resume
       │                        metadata alignment (right company / role)
       │                        employer-consistency warning vs prior submissions
       │                        discrepancy → block + user clarification dialog
       ▼
[ 5. Visual Review Gate ] ──► Typst renders PDF ──► pdftoppm rasterizes every page
       │                      Automated OCR + layout analysis flags:
       │                      overflow, orphan headings, broken wraps,
       │                      placeholder text, wrong role/company names
       ▼
[ 6. Human Approval Gate ] ──► User reviews diff, fact-check report, gap lines, visual flags
       │                       Approved wording auto-promoted to exact_wording rules
       │                       (passive toast confirmation); explicit approve action
       ▼
[ 7. Log & Commit ] ────────► Dual-write: SQLite application_records + applications.csv sync
                              Triggers Git commit via git2
```

### 4.1 Strict Generation Constraints & Rule Types
1. **`never_claim` Engine:** hard negative constraints injected into the system prompt **and** scanned for in the post-hoc fact-check pass. If a JD requests a missing certification or tool, the model is forbidden from adding it; if a draft nonetheless contains a never-claim subject, the draft is blocked.
2. **`exact_wording` Engine:** verbatim preservation for metrics (e.g., "reduced latency from 8 hours to 30 minutes"). No rounding up, no phrase merging, no metric smoothing. Qualifiers like *up to*, *about*, and *6 of 10* are preserved exactly.
3. **`ask_first` Engine:** triggers an interactive user modal prior to draft generation when an unconfirmed tag or early employer is referenced.
4. **`scope_lock` Engine:** prevents scope inflation (e.g., "built an internal script for a team of 4" cannot be rewritten as "led enterprise-wide automation"). Enforced at generation time by prompt constraint and verified post-hoc by the fact-check pass.
5. **`presentation` rules:** their own kind because they shape how true facts appear, not which facts are true — date policy per section, tenure phrasing (e.g., *extensive* instead of a years count), and an employer-neutral toggle for metrics the user does not want traced back to a specific company.

`needs-confirmation` claims are excluded from generation until the user confirms them.

### 4.2 Plain Gap Naming in Cover Letters
Where a job posting requires a skill or certification not present in the Claims Ledger, the system automatically inserts a **Plain Gap Naming** statement into the cover letter draft rather than obscuring it:
> *"While I have not worked with [Required Skill] directly in a production environment, my extensive experience in [Adjacent Strength] allowed me to rapidly deploy similar frameworks..."*

### 4.3 Employer Consistency Guardrail
Repeat applications to the same employer reuse the same record — same facts, same metrics. Before final approval, the fact-check pass queries prior `application_records` for the same employer and compares `claims_used`, metric wording, and full-text similarity; any contradiction between the new draft and an earlier submission raises an **employer consistency warning** in the review gate. The warning is advisory: it alerts the user and never silently rewrites history.

### 4.4 Exact-Wording Approvals
When the user approves a phrasing during review, it is stored verbatim in the `rules` table as an `exact_wording` rule and reused exactly thereafter — approved numbers and scope are never paraphrased. The UI confirms the save with a passive toast ("Wording saved for reuse").

### 4.5 Discrepancy Handling & Clarification
The model proposes; the record decides. Clarification happens at two points:
* **Pre-generation (step 2.5):** ambiguous JD requirements, tiering uncertainty, or ledger gaps surfaced by the analyzer are resolved with the user before any draft is generated.
* **Post-hoc (step 4):** when the fact-check pass finds any discrepancy — an unverified metric, a never-claim subject, a scope inflation, or a metadata mismatch — the draft is blocked from advancing and the user is asked to clarify (confirm, correct, or reject the claim) before the pipeline continues.

In both cases the user confirms, corrects, or rejects; nothing unsupported is smoothed over.

---

## 5. Review Workflow & Quality Gates

### 5.1 Automated Pre-Flight Fact-Check Pass
Before a draft reaches the user, the Rust backend verifies it against the ledger:
* **Hardened metric extraction:** regex extraction of percentages, currency (including K/M/B suffixes), durations (hours, minutes, days, weeks, months, years), multipliers, fractions ("6 of 10"), and plain counts — each captured together with its qualifier ("up to", "about", "approximately", "~"). Any metric not strictly matching a `confirmed_wording` entry in the database is flagged.
* **Never-claim scan:** the draft is scanned for the subjects of all active `never_claim` rules; any hit blocks the draft.
* **Scope verification:** draft phrasing is compared against each claim's confirmed `scope`; scope inflation is flagged.
* **Structural AST diff:** the draft and the master resume are parsed to Markdown ASTs (mdast-style, via `comrak`) and structurally diffed; every change from the base is listed for the user.
* **Metadata alignment:** the exact employer name and job title are verified across the resume header and the cover letter.
* **Employer consistency:** prior submissions to the same employer are compared via `claims_used` IDs, metric wording, and full-text similarity; contradictions raise warnings.
* **Discrepancy flow:** any blocking finding halts the pipeline and opens a clarification dialog; the user confirms, corrects, or rejects before regeneration.

### 5.2 Automated Visual Review Gate
1. Typst compiles `tailored-resume.typ` into `tailored-resume.pdf`.
2. `pdftoppm` rasterizes **every page** to a high-resolution PNG.
3. Layout analysis runs on `pdftotext -bbox` text-layer coordinates: page overflow, orphan headings at page bottoms, broken line wraps, unpopulated template tags (e.g., `[INSERT NAME]`), and confirmation that the letter names the right role and company. Tesseract OCR supplements the analysis for image-only regions.
4. Findings are surfaced as flags in the Visual Review Gate modal. Flags are advisory and require explicit human acknowledgment; blocking fact-check failures keep the approve button disabled.

### 5.3 Human Approval Gate
The user reviews the AST diff, the fact-check report, the gap-naming lines, and the visual flags; approves or edits wording (approvals auto-promote to `exact_wording` rules with a passive toast); and gives an explicit final approve action. Nothing is submitted, emailed, or uploaded without it.

---

## 6. Local Model Integration & Web Scraping Architecture

### 6.1 Local Ollama Engine (`http://localhost:11434`)

Veridicus includes a dedicated **Ollama Model Manager** within the desktop UI that directly controls the local Ollama daemon via its REST endpoints.

1. **Installed Model Discovery (`GET /api/tags`):**
   * Automatically detects and lists local models.
   * Allows assignment of tasks to specific models:
     * **Fast Model (Slash Commands & Fast Analysis):** e.g., `llama3.1:8b`, `qwen2.5:7b` — used for gap analysis, JD scanning, and quick commands.
     * **Deep Model (Resume Tailoring & Mock Interviews):** e.g., `qwen2.5:32b`, `deepseek-r1:14b` — used for guardrailed generation and interview simulation.
2. **Library Search & Pull (`POST /api/pull`):**
   * Built-in search interface to browse recommended models.
   * Real-time streaming progress bar showing download speed (MB/s) and completion percentage, handled by Rust background workers piped to the frontend via Tauri events.

### 6.2 Built-in Job Description HTTP Scraper

When `/apply <URL>` or a UI URL trigger is submitted:

1. **HTTP Request:** The Rust backend uses `reqwest` with custom headers (desktop User-Agent) to issue a `GET` request to the target URL.
2. **Content Extraction:** The HTML document is parsed via the `scraper` crate:
   * Removes non-content elements (`<script>`, `<style>`, `<nav>`, `<footer>`, `<header>`).
   * Extracts target content nodes (main container/article elements).
3. **Markdown Conversion:** Text is converted to formatted Markdown (via `htmd`) and saved locally to `applications/YYYY-MM-DD-company-role/job-description.md`.

---

## 7. Command & Action Execution

Workflows can be initiated via UI buttons or the keyboard command palette (`Cmd+K` / `Ctrl+K`):

* **`/apply <job_url>`:**
  1. Creates the `application_records` row (`PENDING_REVIEW`).
  2. Triggers the HTTP scraper to fetch and save the JD as dated Markdown.
  3. Runs gap analysis: JD split into must-haves / nice-to-haves / gate items, mapped against the Claims Ledger; outputs supported claims, gaps, ask-first items, and ambiguous requirements.
  4. **Pre-generation clarification:** the user resolves ambiguous requirements and ledger gaps before any draft is generated.
  5. Executes guardrailed `resume-tailor` and `cover-letter` skills (confirmed claims + approved wording only).
  6. Runs the automated Fact-Check Pass; discrepancies block and open a clarification dialog.
  7. Compiles `tailored-resume.pdf` via Typst and runs the OCR Visual Review Gate.
  8. On explicit user approval: dual-writes the ledger (SQLite + `applications.csv`) and triggers a local Git commit.
* **`/prep <company>`:** Invokes the Ollama-driven `company-researcher` agent to generate a one-page interview briefing (strategic challenges, culture signals, 5 sharp questions).
* **`/mock <role>`:** Launches the Ollama-driven `interview-simulator` chat modal — STAR-method practice in character as the hiring manager, one question at a time, until the user says "break."
* **`/followup`:** Scans the tracker for non-responses aged 10+ days, drafts follow-up emails, and flags roles where the user should stop waiting.
* **`/weekly`:** Scans the application ledger and outputs a weekly summary report: applications sent, responses received, interviews booked, action items for the week.

**Log analysis:** `daily-log.md` and the application ledger are queryable — e.g., how many applications were submitted today or this week — feeding the `/weekly` report and ad-hoc user queries.

---

## 8. Local Workspace Structure

```
~/Documents/Veridicus/
├── veridicus.db                # SQLCipher-encrypted SQLite (Claims, Rules, Gaps, Apps)
├── SYSTEM.md                  # Master prompt context & system constraints
├── applications.csv           # Human-readable mirror of application_records (synced)
├── daily-log.md               # Append-only execution log; queryable for daily/weekly application counts
├── templates/
│   └── resume_template.typ    # Typst resume layout engine
├── .veridicus/
│   ├── skills/                # Reusable workflow prompts (.md)
│   │   ├── resume-tailor.md
│   │   ├── cover-letter.md
│   │   ├── jd-analyzer.md
│   │   ├── star-builder.md
│   │   └── negotiation-coach.md
│   └── agents/                # Specialized agent definitions (.md)
│       ├── company-researcher.md
│       ├── interview-simulator.md
│       └── application-tracker.md
├── master/
│   └── resume.md              # Source-of-truth master resume
└── applications/              # Dated application folders
    └── YYYY-MM-DD-[company]-[role]/
        ├── job-description.md
        ├── fact-check-report.json
        ├── tailored-resume.md
        ├── tailored-resume.typ
        ├── tailored-resume.pdf
        ├── tailored-resume-preview.png
        ├── cover-letter.md
        └── interview-prep.md
```

---

## 9. Bundled Binaries & Packaging

Most users will not have Typst, Poppler, or an OCR engine on PATH, and compiling them with the MSVC toolchain is a known packaging risk. Veridicus therefore bundles **official prebuilt Windows binaries** as version-pinned sidecars inside the Tauri installer and resolves them from the app bundle at runtime — nothing is compiled with MSVC at build time:

* **`typst`** — PDF compilation (official release binary)
* **`pdftoppm` + `pdftotext` (Poppler)** — page rasterization and text-layer coordinate extraction (`-bbox`) for layout analysis
* **Tesseract OCR** — supplements layout analysis for image-only regions (rare for Typst-generated PDFs, which always have a text layer)

A startup binary check verifies all sidecars are present and executable; the app fails with a clear, actionable error if any sidecar is missing or corrupted.

---

## 10. Implementation Roadmap

1. **Phase 1 (Foundation):** Tauri v2 shell, workspace bootstrap, SQLCipher-encrypted Claims Ledger, rule management including the presentation-rule editor, Git automation.
2. **Phase 2 (Ollama, Scraper & Gap Analyzer):** Ollama model manager with fast/deep model roles, URL scraper, JD gap analysis against the ledger.
3. **Phase 3 (Guardrailed Generation & Verification):** Skill execution engine, hardened fact-check pass with never-claim scanning and AST diffing, Typst compilation, OCR visual gate.
4. **Phase 4 (Integrated Workflows & UI):** Guardrailed `/apply` orchestrator, command palette, interview simulator, application ledger dashboard with CSV↔DB sync.
