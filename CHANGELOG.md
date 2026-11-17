# Changelog

Notable changes to Veridicus, newest first. Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.2] — 2026-11-17

This release replaces the 1.0.1 installers. The application binary changed, so it is a patch bump rather than a re-upload of 1.0.1.

### Fixed

- **ODT export now writes a valid OpenDocument 1.2 document.** `content.xml` was rooted at `<office:document>` instead of `<office:document-content>`, `mimetype` was deflated along with every other part instead of being the first stored entry, the manifest listed no content parts, and the styles used elements that are not in the ODF schema. Readers that validate the container — odfpy among them — reported a document with zero paragraphs, so a resume exported as ODT opened empty even though the text was in the archive. The package now follows ODF 1.2 §3.2, and an exported ODT round-trips through the importer.
- Reading and writing DOCX and ODT moved from `zip` 0.6 to 9.6.1, which clears the `zip` advisories Dependabot reported, including `GHSA-m665-v7pm-7grj` (patched in 9.0.5). Veridicus never called `extract` or `ZipStreamReader`, so the critical path was unreachable, but the crate is now on a patched line.

### Changed

- Frontend dev tooling: Vite 7.3.8, `@vitejs/plugin-react` 5.2.0 and Vitest 4.1.12, which clears the npm advisories without jumping to Vite 8. Nothing in the shipped UI changes.
- The Rust suite is now 210 tests, including DOCX and ODT export/import round trips and a container test for the ODT package.

## [1.0.1] — 2026-11-11

### Fixed

- `/bullets` no longer reads a claim citation as a metric. A draft that cited a claim as `[9abc6037-707e-4531-a654-5947e610666c]` had those digits counted as unbacked numbers, so a correctly grounded bullet was blocked. Citations are now stripped before numbers are compared, and are no longer shown in the rewritten bullet.
- A rewrite must stay on the original bullet's subject. A model that answered a different bullet — replacing a retention achievement with an unrelated, individually grounded claim — used to pass as `rewritten`; it is now refused with `needs_human_input`, because the candidate would be shown a sentence about a different achievement.

### Added

- `src-tauri/eval/demo-bullets.json`, a checked-in model draft used when `VERIDICUS_DEMO_BULLETS` points at it, so the blocked-claim flow can be reproduced for screenshots and recordings without depending on a local model's output. The fixture still passes through every classifier, so it cannot fake a pass the guards would refuse.
- `docs/demo/` with the `/bullets` clip, the blocked-and-rewritten still, the Review claims step and the dashboard, used by the README and the user guide.
- Tests for citation stripping, off-subject rewrites and fixture parsing; the suite is 207 tests.

## [1.0.0] — 2026-11-10

The GitHub release for this version was withdrawn on 2026-11-11, because both installers carried the two `/bullets` defects fixed in 1.0.1. The tag remains, so the source is still there, but there is no download.

### Added

- First public release: Tauri 2 desktop app with a SQLCipher-encrypted claims ledger, the `/apply` pipeline, interview practice, application tracking and the honesty guardrails that ground generated documents in confirmed claims.
- MSI and NSIS installers. Neither is code-signed, so Windows SmartScreen warns on first run.

[1.0.2]: https://github.com/RadioMan18/Veridicus/releases/tag/v1.0.2
[1.0.1]: https://github.com/RadioMan18/Veridicus/releases/tag/v1.0.1
[1.0.0]: https://github.com/RadioMan18/Veridicus/tree/v1.0.0
