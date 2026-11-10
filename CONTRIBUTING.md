# Contributing

Thanks for looking at Veridicus. It is a small, single-maintainer project, so the contributions that
help most are reproducible bug reports, well-argued guardrail cases and focused pull requests.

## Before you start

- A typo, a broken link or an obvious bug: open a pull request.
- Anything that changes behavior: open an issue first and describe the problem and the approach. That
  goes double for the honesty guardrails — a change that makes a document easier to approve needs
  agreement before there is code to review.
- Security issues do not go in issues. See [SECURITY.md](SECURITY.md).
- A wrong sentence, a missed false claim or a false block is an AI concern rather than a bug report; see
  [docs/CONCERNS-CHANNEL.md](docs/CONCERNS-CHANNEL.md).

Be civil. Disagreement about a guardrail is welcome; personal attacks, harassment and bigotry are not.

## Development setup

You need Node 18+, a current stable Rust toolchain, and Ollama running locally with one fast and one
deep model pulled (the README suggests a pair). On Windows, SQLCipher is compiled from source and needs
an OpenSSL install with development headers; the CI workflow shows the environment variables that point
`libsqlite3-sys` at them.

```bash
npm install
npm run tauri dev         # desktop app, Vite hot reload
npm run dev               # frontend only, no Tauri bridge
```

The checks, which are the same ones CI runs:

```bash
npm test                            # Vitest, Node environment, src/**/*.test.ts
npx tsc --noEmit                    # frontend type check
cd src-tauri && cargo test --bins   # guardrails, validators, composition, scrapers
```

`cargo test` covers the Rust side; `npm test` covers the pure frontend modules. Anything that needs the
Tauri bridge or a browser is not covered, so check it in the running app and say in the pull request
what you checked.

Two things to know before a build:

- `src-tauri/Cargo.lock` is ignored on purpose. Do not add it.
- PDF work needs the sidecars in `src-tauri/sidecars/` (`typst`, `pdftoppm`, `pdftotext`). Without them
  the apply pipeline fails at the compile step.

## House rules

**The guardrails are the product.** Never weaken a deterministic check, relax a rule or widen a
template just to make a document pass. If a check is wrong, fix the check and add the fixture that
proves it — the adversarial cases live in `src-tauri/eval/` and in the `cargo test` suites, and a fix
without a failing fixture is an unfinished fix.

**Shipped prompts are versioned.** A shipped skill template in `src-tauri/src/skills.rs` is the whole
system prompt, and its frontmatter carries a `template_version`. Changing a shipped body means bumping
`TEMPLATE_VERSION` and every `template_version` in that file, so existing installs replace their copy
and keep a `.bak`. The `template_tests` fail when a version or a guardrail clause disagrees; edit the
clause list only when the prompt really changed.

**The letter speaks as the applicant.** Cover letters are first person, and pronouns are they/them/their.
Keep that in any prompt or composition change.

**Stay deterministic where you can.** Letterhead, salutation, signature and resume structure are
composed in Rust, not by the model. Extend that instead of asking the model to be more careful.

**Frontend.** React 18, TypeScript and Tailwind, with no new UI library unless there is a reason. Pure
logic belongs in a module with a colocated `*.test.ts`; components are checked by hand.

**Privacy.** Never commit a real resume, ledger, database, `.env`, build log, or a path that names your
machine. `.gitignore` covers the common cases — read `git status` before you commit.

## Commits and pull requests

- One logical change per commit, imperative mood, full stop: "Keep a letter's salutation naming the
  posting's employer."
- Explain why in the body when the diff does not make it obvious.
- A pull request should pass both CI jobs, state the behavior change, and name any documentation it
  makes stale (`README.md`, `USER_GUIDE.md`, `docs/`).
- Screenshots help for UI changes, and it is fine to say what you could only check by eye.

## Licensing

The repository ships an [Apache License 2.0](LICENSE) file. By opening a pull request you agree that
your contribution is licensed under those terms.
