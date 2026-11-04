# CareerOS

Privacy-first job search OS with honesty guardrails. CareerOS is a Tauri 2 desktop application (Rust + React + TypeScript) that helps you manage job applications while enforcing strict \"honesty guardrails\" - only using verified claims from your resume in generated materials.

## Features

- **Honesty Guardrails**: SQLCipher-encrypted Claims Ledger ensures only confirmed facts from your resume are used in generated documents
- **Local AI**: Uses Ollama for local LLM inference (no data sent to external services)
- **Resume Management**: Import resumes (PDF, DOCX, ODT, TXT, MD) and automatically extract claims
- **Job Application Workflow**: Full /apply pipeline - scrape job descriptions, analyze gaps, tailor resume and cover letter, fact-check, compile PDFs
- **ATS Optimization**: Generate ATS-friendly resumes in multiple formats (PDF, DOCX, ODT)
- **Application Tracking**: Built-in ledger to track all applications
- **Interview Prep**: Company research and STAR-method interview simulator
- **Git Integration**: Automatic version control of your job search materials

## Tech Stack

- **Frontend**: React 18 + TypeScript + Vite + Tailwind CSS
- **Backend**: Rust + Tauri 2
- **Database**: SQLite with SQLCipher encryption
- **AI**: Ollama (local LLM)
- **PDF Generation**: Typst
- **PDF Processing**: Poppler (pdftoppm, pdftotext)

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/) (latest stable)
- [Ollama](https://ollama.ai/) installed and running
- At least one Ollama model pulled (e.g., llama3.2:latest)

## Installation

1. Clone the repository
2. Install dependencies: `npm install`
3. Pull an Ollama model: ollama pull llama3.2:latest
4. Run in development: `npm run tauri dev`
5. Build for production: `npm run tauri build`

## Getting Started

1. Launch CareerOS
2. Complete the setup wizard (profile, resume import, claims verification, model selection)
3. Use the command palette (Cmd+K / Ctrl+K) to run slash commands
4. Start applying to jobs with /apply <url>

## Security

- All data stored locally in ~/Documents/CareerOS/
- SQLCipher encryption for the Claims Ledger
- No external API calls by default (only local Ollama)
- Strict guardrails prevent hallucination of unverified claims

## License

Private/Proprietary
