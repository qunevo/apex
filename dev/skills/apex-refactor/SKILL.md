---
name: apex-refactor
description: Audit APEX source and agent instructions for context cost, propose responsibility-based refactorings, then interactively implement approved structural changes while preserving behavior.
---

# Refactor for agent development

Start with root `AGENTS.md`, the [architecture map](../../../app/docs/architecture/README.md) and `python -B dev/scripts/context_audit.py`. Its line counts are navigation hints, not mandatory file-size limits or measured token savings. Audit the requested scope, usually `app/`, plus the instructions and documentation needed to change it. Do not read every large file in full at once.

For each promising hotspot, inspect public entry points, call sites, tests and state ownership. Look for mixed responsibilities, wide dependency fan-out, duplicated contracts, misleading names, stale references and explanations that force readers to load unrelated modules. Distinguish generated schemas, frozen evidence and useful cohesive large files from accidental complexity.

Present a short ranked proposal. For each candidate give its responsibility, the demonstrated navigation/maintenance problem, proposed module boundaries and interfaces, affected callers/tests, expected context benefit and migration risk. Explain when leaving a file intact is better. Ask the user to select or approve a concrete batch before performing broad refactoring. Existing approval of an explicit batch counts; do not repeat it.

Implement on a feature branch following [development](../apex-development/SKILL.md). Extract by responsibility and state boundary, not equal line counts. Preserve public APIs, serialized names, algorithms, ordering, defaults and error behavior unless a separately approved behavior change requires otherwise. Keep independent validation independent. Do not introduce parent dependencies into `app/` or add generic abstraction layers solely to shorten files.

Keep comments about intent, invariants and non-obvious tradeoffs near the owning code. Keep one canonical explanation and short links from other locations. Make root `AGENTS.md` a concise routing/invariant entry; put conditional details in scoped documents. Keep the architecture map aligned with executable names. Avoid duplicating implementation in Markdown or filling code with obvious comments.

Verify each approved batch with the relevant semantic/negative/corrupted-output tests and required Rust checks. Run standalone verification when paths, embedded assets or packaging change. Compare observable behavior where refactoring could alter it. Report changed boundaries, measured file/navigation differences and unresolved risks; do not claim lower token use without a measurement. Propose the next batch only after assessing this result.
