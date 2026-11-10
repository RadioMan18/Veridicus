# Security policy

Veridicus is a local, single-user desktop application. It runs on your machine, keeps the claims
ledger in a SQLCipher-encrypted database under `Documents/Veridicus/`, and talks to nothing except the
job posting URLs you ask it to read and Ollama on localhost. That threat model decides what counts as
a security issue here.

## Reporting a vulnerability

Please do not open a public issue for a security problem.

Use GitHub's private vulnerability reporting on this repository: **Security → Report a vulnerability**.
That opens a private advisory only you and the maintainer can see, and it is the fastest route to a fix.

If that form is unavailable to you, open a normal issue that says only that you have a security report
and how to reach you, without the details.

A useful report says:

- what an attacker gains — read the ledger, tamper with a generated document, run code, send text off
  the machine
- the version and installer you tested (MSI or NSIS; the installer filename carries the version)
- the smallest reproduction you can give — a command, a workspace state, a crafted import or a posting
  URL that triggers it
- how much depends on the local Ollama models you have pulled

## In scope

- **The ledger at rest.** SQLCipher key derivation and storage, the key file in the workspace dot
  directory, and anything that lets one account or process read another user's database.
- **The Tauri boundary.** IPC commands that accept a path, URL or Markdown, the CSP, the `externalBin`
  sidecar invocation, and the file dialogs.
- **Untrusted input.** A job posting, imported resume or ledger field that escapes its role as data:
  HTML parsing, prompt injection that changes the deterministic rules, path traversal from a scraped
  URL, or a Markdown/Typst injection that reaches the PDF pipeline.
- **Exports.** DOCX, ODT and ZIP writing, and the Typst renderer, including a crafted document that
  writes outside the application folder.
- **Data egress.** A default configuration, a log or a notification that sends document text somewhere
  other than localhost.

## Out of scope

- **A bad sentence no guardrail claims to catch.** A false claim that the deterministic checks let
  through is a correctness bug, not a vulnerability, unless you can show it bypasses a check that should
  have blocked it.
- **A document approved after a written override.** The override is a deliberate human decision and is
  recorded as one.
- **SmartScreen or antivirus warnings about the unsigned installers.** They are not code-signed yet;
  that is known and documented.
- **Attacks that need administrator rights, physical access, or an already-compromised account.** There
  is no privilege boundary to cross inside a single-user desktop app.
- **Bugs in Ollama, Typst or Poppler themselves.** Report those upstream. Do tell us if Veridicus
  invokes them in a way that makes the bug worse.

## Supported versions

Only the latest release (currently 1.x) receives security fixes. Pre-1.0 builds are unsupported and
should be upgraded.

## What to expect

Veridicus is maintained by one person on a best-effort basis, with no security team and no bug bounty.
You can expect an acknowledgement within about a week, an assessment of severity and affected versions,
and a fix or a documented mitigation in the next release. Please allow a reasonable window before public
disclosure; 90 days from the report is the default, and it can be shorter once a fix is available.
Credit is offered in the advisory unless you would rather stay anonymous.

## Wrong output is a different kind of report

A misleading document, a missed false claim or a false block is an AI concern, not a vulnerability.
Handle those through the in-app **Report a problem with this output** action, which records an incident
with an id and status in the workspace, or through the procedure in
[docs/CONCERNS-CHANNEL.md](docs/CONCERNS-CHANNEL.md). Both routes are recorded and neither involves
retaliation.
