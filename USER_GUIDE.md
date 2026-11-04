# CareerOS User Guide

## Getting Started

### First Launch
1. Launch CareerOS from your installed application
2. Complete the Setup Wizard:
   - **Welcome**: Introduction to CareerOS
   - **Profile**: Enter your name, contact info, and target roles
   - **Master Resume**: Upload or paste your resume
   - **Claims Verification**: Review and confirm extracted claims (these become your verified facts)
   - **Ollama Models**: Select models for fast and deep tasks

### Opening the Command Palette
Press Cmd+K (Mac) or Ctrl+K (Windows/Linux) to open the command palette, or click the \"Cmd+K\" button in the header.

## Workflow Order

### 1. Import Your Resume
Before applying to jobs, you need to import your resume and build your Claims Ledger.

**Option A: Dashboard Button**
- Click \"Open Resume File\" in the header
- Select your resume (PDF, DOCX, ODT, TXT, or MD)

**Option B: Command Palette**
`
/resume [filepath]
`
- If no filepath provided, a file dialog will open
- Claims are automatically extracted and saved to the encrypted ledger

### 2. Apply to a Job
The main workflow for applying to a job posting.

`
/apply <job-url>
`

**What it does:**
1. Scrapes the job description from the URL
2. Runs gap analysis against your Claims Ledger
3. Generates a tailored resume using only confirmed claims
4. Generates a cover letter using only confirmed claims
5. Runs fact-check pass to ensure no hallucinations
6. Compiles PDF using Typst
7. Performs visual QA on generated PDFs
8. Updates Application Ledger and commits to Git

**Example:**
`
/apply https://jobs.lever.co/company/abc123
`

### 3. Prepare for Interviews
Research a company before your interview.

`
/prep <company-name>
`

**Example:**
`
/prep Acme Corp
`

Generates a company briefing with overview, recent news, culture, interview tips, and talking points.

### 4. Practice Interviews
Run a STAR-method interview simulator.

`
/mock <role>
`

**Example:**
`
/mock Senior Software Engineer
`

Starts an interactive interview with a hiring manager persona, asking behavioral questions one at a time with feedback.

### 5. Follow Up on Applications
Draft follow-up emails for stale applications.

`
/followup
`

Finds applications in SUBMITTED or INTERVIEWING status and generates follow-up draft emails.

### 6. Weekly Report
Generate a summary of your job search activity.

`
/weekly
`

Shows total applications, submitted, interviewing, rejected, and offers.

### 7. Log Analysis
Analyze your daily logs and application counts.

`
/logs
`

Shows today's applications, past 7 days, and daily log contents.

## Additional Features

### Application Ledger
Click \"Application Ledger\" in the header to view all applications with their status, employer, role, and dates. You can update application status from this view.

### Export Resume
Click \"Export Resume\" to export your resume in ATS-optimized format (PDF, DOCX, or ODT).

### Ollama Settings
Click \"Ollama Settings\" to:
- View installed models
- Configure fast model (quick tasks) and deep model (complex tasks like /apply)
- Download new models from Ollama

## Tips

- **Be honest**: Only claims in your ledger are used. If a required skill isn't there, it won't be added
- **Keep Ollama running**: CareerOS requires Ollama to be running locally
- **Use the ledger**: Track all applications in the Application Ledger
- **Git history**: All /apply runs are automatically committed to Git in your CareerOS workspace

## Troubleshooting

- **Ollama not responding**: Ensure Ollama is running (ollama serve) and the model is loaded
- **Resume import fails**: Check file format (PDF, DOCX, ODT, TXT, MD supported)
- **/apply fails**: Ensure resume is imported, claims are verified, and Ollama model is configured
- **PDF generation fails**: Check that Typst sidecar is present
