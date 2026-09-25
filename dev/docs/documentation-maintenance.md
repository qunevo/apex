# Documentation and context maintenance

Executable behavior, documented intent and measured evidence are different sources of truth. Use the architecture change map to find affected contracts and tests. When they disagree, identify whether the code or the claim is wrong before editing either. A link checker cannot establish semantic consistency.

| Material | Owner and maintenance rule |
| --- | --- |
| Product contracts and operation | `app/docs`; keep usable in a standalone app checkout |
| Module boundaries and affected tests | `app/docs/architecture/README.md`; link to owners instead of duplicating implementation |
| Customer planning/customization instructions | `app/skills`; never depend on repository contributor tools |
| Contributor processes and CI/release policy | `dev/docs` and `dev/skills`; discovery entries are generated |
| Historical migration evidence | `dev/docs/migration-audit.md`; preserve dates, versions and limits |
| Frozen benchmark evidence | `benchmark/`; no cleanup without specific review of the frozen artifact |
| Local reports, caches and runtime artifacts | ignored local outputs; establish ownership and reproducibility before cleanup |

Run `python -B dev/scripts/context_audit.py` for a small ranked list of source and Markdown hotspots. Its thresholds prompt investigation; they are not style limits. Inspect mixed responsibilities and the files a developer must load for a concrete change. Keep cohesive code together, split only along useful interfaces, and keep comments focused on invariants and reasoning.

The [refactoring skill](../skills/apex-refactor/SKILL.md) proposes and executes approved batches. The [documentation skill](../skills/apex-docs/SKILL.md) compares material claims to implementation and tests. The [artifact skill](../skills/apex-artifacts/SKILL.md) maintains a reviewable provenance/removal list. None is an unattended instruction to rewrite the whole application or delete old evidence.

CI runs local Markdown destination checks, developer-skill discovery synchronization checks, wiki allowlist/link validation and publication tests. Use `python -B dev/scripts/check_docs.py`, `python -B dev/scripts/sync_skills.py --check`, and `python -B dev/scripts/build_wiki.py` locally. These checks catch structural drift; meaningful command examples and claims still require relevant executable verification. The wiki is generated only from the tested main revision.

For an unfinished checkout with new, untracked source files, run `python -B dev/scripts/check_source.py`. It validates those checks in a temporary source snapshot without changing the original Git index. The snapshot includes non-ignored source files, excludes local generated output, and is removed afterward. Normal wiki publication still requires tracked source files.

Keep a plan only while it supports an active task or a durable architectural decision. Consolidate lasting contracts in their owning documents and remove superseded working notes within the approved cleanup scope. Prefer short indexes and focused references over accumulating root instructions or duplicated summaries.
