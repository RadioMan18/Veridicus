# CareerOS Desktop — Phased Implementation Plan

This document outlines the step-by-step engineering plan for building **CareerOS Desktop**. Construction is divided into **4 distinct phases**. Every phase concludes with an explicit **HITL (Human-In-The-Loop) Quality & Testing Gate**. Development cannot proceed to the next phase until all automated tests pass and manual verification criteria are met.

---

## Technical Stack Overview

* **Desktop Shell:** Tauri v2 (Rust backend, React + TypeScript frontend)
* **Styling & UI:** Tailwind CSS, Lucide Icons, Radix UI / Shadcn primitives
* **Inference Engine:** Local Ollama Daemon REST API (`http://localhost:11434`)
* **Web Scraping:** Rust (`reqwest` + `scraper` + `htmd` for HTML-to-Markdown)
* **Document Compilation:** Typst CLI / Embedded Engine
* **Storage & Audit:** Local File System (`~/Documents/CareerOS/`) + Embedded Git (`git2` crate)

---

## Phase 1: Core Shell, File System & Git Infrastructure

### Objective
Establish the Tauri v2 desktop application workspace, state store, directory structure, local file system manager, and Git repository hooks.

### Implementation Tasks

1. **Initialize Project Infrastructure:**
   * Create Tauri v2 app with React + TypeScript frontend (`bun create tauri-app`).
   * Configure Rust backend dependencies in `src-tauri/Cargo.toml`: `git2`, `tokio`, `serde`, `serde_json`, `dirs`.

2. **Workspace Setup & System Initialization (`src-tauri/src/workspace.rs`):**
   * Implement automated directory bootstrap logic for `~/Documents/CareerOS/`.
   * Create missing base folders (`templates/`, `.careeros/skills/`, `.careeros/agents/`, `master/`, `applications/`).
   * Seed default system prompts (`SYSTEM.md`, default `applications.csv`).

3. **Git Integration Engine (`src-tauri/src/git_engine.rs`):**
   * Initialize local Git repository inside `~/Documents/CareerOS/` using `git2`.
   * Expose Rust command to auto-commit changes after application logs or profile updates.

### Key Code Implementation

```rust:src-tauri/src/workspace.rs
use std::fs;
use std::path::{Path, PathBuf};
use git2::Repository;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct WorkspaceStatus {
    pub root_path: String,
    pub initialized: bool,
    pub git_active: bool,
}

#[tauri::command]
pub fn initialize_workspace() -> Result<WorkspaceStatus, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("CareerOS");

    let subdirs = vec![
        "templates",
        ".careeros/skills",
        ".careeros/agents",
        "master",
        "applications",
    ];

    for dir in subdirs {
        let path = workspace_root.join(dir);
        if !path.exists() {
            fs::create_dir_all(&path).map_err(|e| format!("Failed to create {:?}: {}", path, e))?;
        }
    }

    // Seed master application ledger if missing
    let csv_path = workspace_root.join("applications.csv");
    if !csv_path.exists() {
        let headers = "id,date,company,role,status,job_url,folder_path\n";
        fs::write(&csv_path, headers).map_err(|e| e.to_string())?;
    }

    // Seed SYSTEM.md master prompt if missing
    let system_md = workspace_root.join("SYSTEM.md");
    if !system_md.exists() {
        let content = "# Master Career Context\n\n## Target Roles\n- Senior Product Manager\n\n## Tone Constraints\n- Direct, active voice, zero jargon.\n";
        fs::write(&system_md, content).map_err(|e| e.to_string())?;
    }

    // Initialize Git repository
    let git_active = match Repository::open(&workspace_root) {
        Ok(_) => true,
        Err(_) => Repository::init(&workspace_root).is_ok(),
    };

    Ok(WorkspaceStatus {
        root_path: workspace_root.to_string_lossy().into_owned(),
        initialized: true,
        git_active,
    })
}
```

---

### Phase 1 Testing & HITL Quality Gate

#### Automated Test Suite
- [ ] **FS Unit Tests:** Verify `initialize_workspace` creates all mandatory directories and files without throwing errors.
- [ ] **Git Integration Tests:** Verify programmatic commits succeed and return valid commit hashes.

#### Manual Verification Checklist (HITL Gate)
- [ ] App launches natively without webview or Tauri IPC panic errors.
- [ ] Verify `~/Documents/CareerOS/` directory tree is correctly populated upon first launch.
- [ ] Confirm `applications.csv` and `SYSTEM.md` exist and are populated with default headers/content.
- [ ] Run `git status` inside `~/Documents/CareerOS/` via terminal and verify repository is active.

**CRITICAL:** Phase 1 must achieve 100% test pass rate and user confirmation before moving to Phase 2.

---

## Phase 2: Local Ollama Engine & Web Scraper Integration

### Objective
Connect CareerOS to the local Ollama daemon for model discovery, selection, and model downloading, while integrating a Rust-native HTTP scraper to convert target job posting URLs directly into structured Markdown files.

### Implementation Tasks

1. **Ollama REST Client (`src-tauri/src/ollama.rs`):**
   * Implement endpoints for listing models (`/api/tags`), checking daemon health, and generating responses (`/api/generate` or `/api/chat`).
   * Build model pull/download function with chunked event streaming (`/api/pull`) piped to Tauri frontend via Tauri Events.

2. **Web Scraper & Markdown Converter (`src-tauri/src/scraper.rs`):**
   * Use `reqwest` to issue GET requests with customized user-agent strings.
   * Parse HTML using `scraper`, filter out noise tags (`<nav>`, `<style>`, `<script>`, `<footer>`).
   * Clean remaining text into formatted Markdown.

### Key Code Implementation

```rust:src-tauri/src/scraper.rs
use reqwest::header::USER_AGENT;
use scraper::{Html, Selector};
use std::fs;
use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct ScrapedJob {
    pub url: String,
    pub title: String,
    pub raw_markdown: String,
    pub saved_path: String,
}

#[tauri::command]
pub async fn scrape_job_url(url: String, company: String, role: String) -> Result<ScrapedJob, String> {
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header(USER_AGENT, "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let html_content = response.text().await.map_err(|e| format!("Failed reading response: {}", e))?;
    let document = Html::parse_document(&html_content);

    // Extract basic title or fall back to role input
    let title_selector = Selector::parse("title").unwrap();
    let page_title = document
        .select(&title_selector)
        .next()
        .map(|e| e.text().collect::<String>())
        .unwrap_or_else(|| format!("{} at {}", role, company));

    // Strip noise and extract main body text
    let body_selector = Selector::parse("body").unwrap();
    let body_text = match document.select(&body_selector).next() {
        Some(element) => element.text().collect::<Vec<_>>().join(" "),
        None => html_content.clone(),
    };

    // Clean whitespace and build markdown structure
    let cleaned_text = body_text.split_whitespace().collect::<Vec<&str>>().join(" ");
    let markdown_content = format!(
        "# {} - {}\n\n**Source URL:** {}\n\n## Job Description\n\n{}",
        company, role, url, cleaned_text
    );

    // Save to job folder
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let timestamp = chrono::Local::now().format("%Y-%m-%d").to_string();
    let folder_name = format!("{}-{}-{}", timestamp, company.to_lowercase(), role.to_lowercase())
        .replace(' ', "-");
    
    let target_dir = docs_dir.join("CareerOS").join("applications").join(&folder_name);
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    let jd_file_path = target_dir.join("job-description.md");
    fs::write(&jd_file_path, &markdown_content).map_err(|e| e.to_string())?;

    Ok(ScrapedJob {
        url,
        title: page_title,
        raw_markdown: markdown_content,
        saved_path: jd_file_path.to_string_lossy().into_owned(),
    })
}
```

```typescript:src/components/OllamaManager.tsx
import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface ModelTag {
  name: string;
  size: number;
  modified_at: string;
}

export const OllamaManager: React.FC = () => {
  const [models, setModels] = useState<ModelTag[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>('');
  const [downloadName, setDownloadName] = useState<string>('');
  const [progress, setProgress] = useState<string>('');

  useEffect(() => {
    fetchModels();
    
    // Listen for model download progress events from Rust backend
    const unlisten = listen<string>('ollama-pull-progress', (event) => {
      setProgress(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const fetchModels = async () => {
    try {
      const availableModels = await invoke<ModelTag[]>('get_ollama_models');
      setModels(availableModels);
      if (availableModels.length > 0) setSelectedModel(availableModels[0].name);
    } catch (err) {
      console.error('Ollama offline or unreachable:', err);
    }
  };

  const handlePullModel = async () => {
    if (!downloadName) return;
    try {
      await invoke('pull_ollama_model', { modelName: downloadName });
      fetchModels();
      setDownloadName('');
    } catch (err) {
      console.error('Download failed:', err);
    }
  };

  return (
    <div className="p-4 bg-slate-900 text-white rounded-xl shadow-md">
      <h2 className="text-xl font-bold mb-4">Ollama Model Manager</h2>
      <div className="mb-4">
        <label className="block text-sm mb-1 text-slate-400">Active Inference Model</label>
        <select 
          value={selectedModel} 
          onChange={(e) => setSelectedModel(e.target.value)}
          className="w-full p-2 bg-slate-800 rounded border border-slate-700"
        >
          {models.map((m) => (
            <option key={m.name} value={m.name}>{m.name}</option>
          ))}
        </select>
      </div>

      <div className="mt-6 pt-4 border-t border-slate-800">
        <label className="block text-sm mb-1 text-slate-400">Download New Model (e.g. qwen2.5:7b)</label>
        <div className="flex gap-2">
          <input 
            type="text" 
            value={downloadName} 
            onChange={(e) => setDownloadName(e.target.value)} 
            placeholder="qwen2.5:7b" 
            className="flex-1 p-2 bg-slate-800 rounded border border-slate-700"
          />
          <button 
            onClick={handlePullModel} 
            className="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded font-medium"
          >
            Pull Model
          </button>
        </div>
        {progress && <p className="text-xs text-blue-400 mt-2">{progress}</p>}
      </div>
    </div>
  );
};
```

---

### Phase 2 Testing & HITL Quality Gate

#### Automated Test Suite
- [ ] **Ollama Integration Test:** Test `http://localhost:11434/api/tags` connection and handle daemon offline gracefully with clear user error.
- [ ] **Scraper Integration Test:** Test scraper command against a sample public job listing URL; confirm invalid URLs return appropriate error messages.

#### Manual Verification Checklist (HITL Gate)
- [ ] Open CareerOS, confirm installed local Ollama models appear in dropdown.
- [ ] Pull a lightweight test model (e.g., `qwen2.5:0.5b` or `tinyllama`) through the UI and verify streaming progress updates properly.
- [ ] Input a public job posting URL (e.g., a greenhouse.io or lever.co posting), execute scrape, and verify clean Markdown file creation in `~/Documents/CareerOS/applications/.../job-description.md`.

**CRITICAL:** Phase 2 must achieve 100% test pass rate and user confirmation before moving to Phase 3.

---

## Phase 3: Typst Resume Engine & Skill Execution Engine

### Objective
Build the document generation pipeline using **Typst** and implement the execution engine for custom prompt skills (`resume-tailor`, `cover-letter`, `jd-analyzer`).

### Implementation Tasks

1. **Typst Compiler Driver (`src-tauri/src/typst_engine.rs`):**
   * Invoke `typst compile` CLI or embedded bindings to compile `.typ` documents into `.pdf`.
   * Include standard default templates (`templates/resume_template.typ`).

2. **Skill Engine Orchestrator (`src-tauri/src/skills.rs`):**
   * Load system prompts (`SYSTEM.md`), master resume (`master/resume.md`), and selected skill template (`.careeros/skills/resume-tailor.md`).
   * Construct structured prompt, invoke active Ollama model, extract Markdown, write `.typ` output, and compile final PDF.

### Key Code Implementation

```rust:src-tauri/src/typst_engine.rs
use std::process::Command;
use std::path::Path;

#[tauri::command]
pub fn compile_typst_pdf(typst_file_path: String) -> Result<String, String> {
    let input_path = Path::new(&typst_file_path);
    if !input_path.exists() {
        return Err(format!("Typst file not found: {}", typst_file_path));
    }

    let output_path = input_path.with_extension("pdf");

    let output = Command::new("typst")
        .arg("compile")
        .arg(input_path)
        .arg(&output_path)
        .output()
        .map_err(|e| format!("Failed to execute Typst binary: {}. Ensure Typst is installed.", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Typst Compilation Failed:\n{}", stderr));
    }

    Ok(output_path.to_string_lossy().into_owned())
}
```

```typst:templates/resume_template.typ
// Standard ATS-Friendly Typst Resume Template
#set page(paper: "us-letter", margin: (x: 1.5cm, y: 1.5cm))
#set text(font: "Liberation Sans", size: 10pt)

#let resume_header(name: "", email: "", phone: "", linkedin: "") = {
  align(center)[
    #text(size: 18pt, weight: "bold")[#name] \
    #v(-4pt)
    #text(size: 9pt, fill: rgb("#444444"))[
      #email | #phone | #linkedin
    ]
  ]
  #v(5pt)
}

// Content placeholder for generated resume
#resume_header(
  name: "Jane Doe",
  email: "jane.doe@example.com",
  phone: "(555) 019-2834",
  linkedin: "linkedin.com/in/janedoe"
)

== Professional Summary
Results-driven Product Manager with 7+ years of experience in fintech, scaling growth platforms, and leading cross-functional engineering teams.

== Core Capabilities
- Product Strategy & Roadmap Execution
- Data Analytics & Metric Optimization
- Stakeholder & Cross-Functional Alignment
```

---

### Phase 3 Testing & HITL Quality Gate

#### Automated Test Suite
- [ ] **Typst Binary Check:** Rust test verifying local `typst` binary is reachable via system PATH.
- [ ] **Compilation Test:** Programmatically compile `templates/resume_template.typ` to PDF and verify the resulting file exists and is >0 bytes.

#### Manual Verification Checklist (HITL Gate)
- [ ] Provide a test JD and master resume; run `resume-tailor` workflow.
- [ ] Verify Ollama model generates tailored output without hallucinating non-existent work history.
- [ ] Confirm output `.typ` compiles to `.pdf` seamlessly in under 500ms.
- [ ] Open compiled `.pdf` in a PDF viewer and verify layout, typography, and page boundaries are clean.

**CRITICAL:** Phase 3 must achieve 100% test pass rate and user confirmation before moving to Phase 4.

---

## Phase 4: Integrated Workflow Automation, Command Palette & UI Polish

### Objective
Assemble the `/apply` composite workflow, implement global command palette (`Cmd+K` / `Ctrl+K`), build interactive interview simulation agent, and create application ledger dashboard with auto-committing Git hooks.

### Implementation Tasks

1. **Global Command Palette Component (`src/components/CommandPalette.tsx`):**
   * Modal overlay listening for shortcut (`Cmd+K` / `Ctrl+K`).
   * Quick execution triggers: `/apply <url>`, `/prep <company>`, `/mock <role>`, `/weekly`.

2. **Full `/apply` Automation Orchestrator (`src-tauri/src/orchestrator.rs`):**
   * Sequential execution pipeline:
     1. Scrape JD URL to Markdown.
     2. Execute `jd-analyzer` skill via Ollama.
     3. Execute `resume-tailor` skill and compile PDF via Typst.
     4. Execute `cover-letter` skill.
     5. Append record to `applications.csv`.
     6. Trigger automatic Git commit via `git2`.

3. **Application Ledger UI (`src/components/ApplicationLedger.tsx`):**
   * Data table parsing local `applications.csv`.
   * Filters for application status (`PENDING_REVIEW`, `SUBMITTED`, `INTERVIEWING`, `REJECTED`, `OFFER`).
   * Quick button to launch application directory in local system file explorer.

### Key Code Implementation

```typescript:src/components/CommandPalette.tsx
import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
}

export const CommandPalette: React.FC<CommandPaletteProps> = ({ isOpen, onClose }) => {
  const [command, setCommand] = useState('');
  const [status, setStatus] = useState<string>('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        if (isOpen) onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const handleExecute = async () => {
    if (!command.trim()) return;
    setLoading(true);
    setStatus('Executing workflow...');

    try {
      if (command.startsWith('/apply ')) {
        const url = command.replace('/apply ', '').trim();
        await invoke('execute_apply_workflow', { url });
        setStatus('✓ Application package created! Review generated files in workspace.');
      } else if (command.startsWith('/weekly')) {
        const report = await invoke<string>('generate_weekly_report');
        setStatus(report);
      } else {
        setStatus('Unknown command. Available commands: /apply <url>, /weekly');
      }
    } catch (err) {
      setStatus(`Error: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 z-50">
      <div className="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-2xl overflow-hidden shadow-2xl">
        <div className="p-4 border-b border-slate-800 flex items-center gap-2">
          <span className="text-blue-400 font-mono font-bold">&gt;_</span>
          <input
            type="text"
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleExecute()}
            placeholder="Type a slash command, e.g. /apply https://company.com/job..."
            className="w-full bg-transparent text-white outline-none font-mono"
            autoFocus
          />
        </div>
        <div className="p-4 bg-slate-950 text-sm font-mono min-h-[100px] text-slate-300">
          {loading ? <p className="animate-pulse text-blue-400">Processing with local model...</p> : <p>{status || 'Ready for command input.'}</p>}
        </div>
        <div className="p-2 bg-slate-900 border-t border-slate-800 text-xs text-slate-500 flex justify-between">
          <span>Press <strong>Enter</strong> to run</span>
          <span>Press <strong>Esc</strong> to exit</span>
        </div>
      </div>
    </div>
  );
};
```

---

### Phase 4 Testing & HITL Quality Gate

#### Automated Test Suite
- [ ] **End-to-End Test (`/apply`):** Test end-to-end flow with a mock server endpoint supplying HTML content; verify JD file saved, model executed, Typst compiled PDF, `applications.csv` updated, and Git commit created.
- [ ] **Ledger CSV Parsing Test:** Verify parsing of malformed CSV lines without crashing UI.

#### Manual Verification Checklist (HITL Gate)
- [ ] Trigger command palette using shortcut (`Cmd+K` / `Ctrl+K`).
- [ ] Run `/apply <real_job_url>` and confirm the full sequence completes seamlessly:
  1. Job text scraped to Markdown.
  2. Match analysis complete.
  3. Tailored resume PDF generated via Typst.
  4. Cover letter drafted.
  5. Application appended to `applications.csv`.
  6. Automatic Git commit created.
- [ ] Verify `applications.csv` entry displays in application ledger dashboard.
- [ ] Open local git log (`git log -n 1`) inside `~/Documents/CareerOS/` and confirm auto-commit entry for the job application exists.

**FINAL RELEASE GATE:** System passes full local execution test without external network API leaks. Project complete.