# Technical Specification: CareerOS Desktop

**System Architecture & Technical Design for a Standalone Desktop Job Search OS powered by Tauri & Local Ollama LLMs**

## 1. Executive Overview & System Architecture

**CareerOS** is a privacy-first, desktop-native application designed to transform job search workflows into structured, file-based projects. Built with **Tauri v2** (Rust backend + React/TypeScript frontend) and integrated directly with a local **Ollama** runtime, CareerOS eliminates reliance on third-party SaaS tools or cloud AI API keys.

The application translates prompt engineering patterns (context rules, skill templates, agent roles, and workflow triggers) into a local desktop interface equipped with a local model manager, system-level file management, background task queues, and PDF processing pipelines via **Typst**.

```
+-----------------------------------------------------------------------------------+
|                                 CareerOS Desktop                                  |
|                                                                                   |
|  +-----------------------------------------------------------------------------+  |
|  |                           React / TS UI Frontend                            |  |
|  |  +-------------------+  +---------------------+  +-----------------------+  |  |
|  |  | Profile & Rules   |  | Workflow Dashboard  |  | Ollama Model Library  |  |  |
|  |  | Editor (CLAUDE)   |  | & Quick Trigger     |  | Search & Downloader   |  |  |
|  |  +-------------------+  +---------------------+  +-----------------------+  |  |
|  +-------------------------------------+---------------------------------------+  |
|                                        | Tauri IPC Communication                  |
|  +-------------------------------------v---------------------------------------+  |
|  |                           Tauri Rust Core Backend                           |  |
|  |  +--------------------+  +--------------------+  +-----------------------+  |  |
|  |  | File System & Git  |  | Task Orchestrator  |  | Local Model Manager   |  |  |
|  |  |  + Typst Compiler  |  | & HTTP Scraper     |  | & Download Streamer   |  |  |
|  |  +--------------------+  +--------------------+  +-----------------------+  |  |
|  +-------------------------------------+---------------------------------------+  |
+----------------------------------------|------------------------------------------+
                                         | HTTP / REST (http://localhost:11434)
                                         v
+-----------------------------------------------------------------------------------+
|                                 Ollama Service                                    |
|  +---------------------------+  +----------------------------------------------+  |
|  | Model Registry            |  | Local Inference Engine                       |  |
|  | (/api/tags, /api/pull)    |  | (/api/chat, /api/generate)                   |  |
|  +---------------------------+  +----------------------------------------------+  |
+-----------------------------------------------------------------------------------+
```

---

## 2. Tech Stack & Key Specifications

| Component | Technology | Description & Implementation |
| :--- | :--- | :--- |
| **Framework** | **Tauri v2** | Lightweight, cross-platform container utilizing Rust for OS level operations and web views for rendering. |
| **Frontend UI** | **React + Tailwind CSS** | Application dashboard, visual editor, model manager, and interactive command palette (`Cmd+K` / `Ctrl+K`). |
| **Local LLM Engine** | **Ollama REST API** | Interfacing directly with `http://localhost:11434` for model search, pull/download streaming, and prompt completion. |
| **Job Scraper** | **Rust (`reqwest` + `scraper`)** | Built-in HTTP client to fetch public web pages, extract HTML, strip boilerplate, and convert main job text to clean Markdown. |
| **Document Compiler** | **Typst CLI / Library** | Lightning-fast, local markup compilation system used to render tailored Markdown resumes into professional, ATS-friendly PDFs. |
| **Storage & Audit** | **Local FS + Git** | Workspace hosted at `~/Documents/CareerOS/` structured with Markdown files, CSV ledger (`applications.csv`), and auto-commits via local Git. |

---

## 3. Data & Storage Model

All application data lives locally on the user's hard drive in a dedicated workspace folder (`~/Documents/CareerOS/` by default).

```
~/Documents/CareerOS/
├── SYSTEM.md                  # Master prompt context & system constraints
├── applications.csv           # Structured ledger of active and past applications
├── daily-log.md               # Automated execution log and activity summary
├── templates/
│   └── resume_template.typ    # Typst styling layout for PDF exports
├── .careeros/
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
└── applications/              # Application workspace folders
    └── YYYY-MM-DD-[company]-[role]/
        ├── job-description.md
        ├── tailored-resume.md
        ├── tailored-resume.typ
        ├── tailored-resume.pdf
        ├── cover-letter.md
        └── interview-prep.md
```

---

## 4. Local Model Integration & Web Scraping Architecture

### 4.1 Local Ollama Engine (`http://localhost:11434`)

CareerOS includes a dedicated **Ollama Model Manager** within the desktop UI that directly controls the local Ollama daemon via its REST endpoints.

1. **Installed Model Discovery (`GET /api/tags`):**
   * Automatically detects and lists local models.
   * Allows assignation of tasks to specific models:
     * **Fast Model (Slash Commands & Fast Analysis):** e.g., `llama3.1:8b`, `qwen2.5:7b`.
     * **Deep Model (Resume Tailoring & Mock Interviews):** e.g., `qwen2.5:32b`, `deepseek-r1:14b`.
2. **Library Search & Pull (`POST /api/pull`):**
   * Built-in search interface to browse recommended models.
   * Real-time streaming progress bar showing download speed (MB/s) and completion percentage handled by Rust background workers.

### 4.2 Built-in Job Description HTTP Scraper

When `/apply <URL>` or a UI URL trigger is submitted:

1. **HTTP Request:** The Rust backend uses `reqwest` with custom headers (desktop User-Agent) to issue a `GET` request to the target URL.
2. **Content Extraction:** The HTML document is parsed via the `scraper` crate:
   * Removes non-content elements (`<script>`, `<style>`, `<nav>`, `<footer>`, `<header>`).
   * Extracts target content nodes (main container/article elements).
3. **Markdown Conversion:** Text is converted to formatted Markdown and saved locally to `applications/YYYY-MM-DD-company-role/job-description.md`.

---

## 5. Core Architectural Workflows

### 5.1 PDF Compilation Pipeline (Typst)

```
[ Tailored Markdown ] ---> [ Rust Typst Engine ] ---> [ Typst Template (.typ) ] ---> [ ATS-Friendly PDF ]
```

1. **Generation:** The `resume-tailor` skill produces a structured Markdown version of the tailored resume.
2. **Conversion:** CareerOS maps the Markdown output into a Typst template file (`tailored-resume.typ`).
3. **Compilation:** The embedded Typst CLI compiles `tailored-resume.typ` into `tailored-resume.pdf` in under 100ms without cloud dependencies or external browser instances.

### 5.2 Command & Action Execution (`/apply`, `/prep`, `/mock`, `/weekly`)

Workflows can be initiated via UI buttons or the keyboard command palette (`Cmd+K` / `Ctrl+K`):

* **`/apply <job_url>`:**
  1. Triggers HTTP Scraper to fetch and save job text as Markdown.
  2. Runs `jd-analyzer` skill using the local Ollama model to highlight key skills and red flags.
  3. Executes `resume-tailor` to match applicant experience against JD constraints.
  4. Passes output to **Typst** engine to compile `tailored-resume.pdf`.
  5. Runs `cover-letter` skill to generate a tailored 250-word letter.
  6. Appends entry to `applications.csv` with status `PENDING_REVIEW` and triggers a local Git commit.
* **`/prep <company>`:** Invokes the `company-researcher` subagent to generate an interview briefing.
* **`/mock <role>`:** Launches an interactive STAR method interview practice modal in character as the hiring manager.
* **`/weekly`:** Scans `applications.csv` and outputs a weekly summary report.

---

## 6. Implementation Roadmap

1. **Phase 1 (Foundation):** Setup Tauri v2 desktop shell, React frontend state management, local workspace file creator, and Git automation hooks.
2. **Phase 2 (Ollama Manager & Scraper):** Implement REST client for Ollama management (`/api/tags`, `/api/pull`) and `reqwest`/`scraper` pipeline for URL fetching.
3. **Phase 3 (Typst & Workflows):** Integrate Typst compilation engine, build skill prompt parsers, and execute `/apply` automated workflow.
4. **Phase 4 (UI Polish & Features):** Build command palette, interview simulator UI, and interactive application ledger view.