---
name: apex-docs
description: Reconcile APEX documentation and agent instructions with implemented behavior, repair references, and keep the public wiki sources accurate without duplicating contracts.
---

# Maintain current documentation

Scope the affected feature or documentation area first. Compare the claims to executable entry points, typed input, validation and relevant tests. Code is evidence of behavior, not automatic proof that the behavior fulfills the intended contract. Distinguish implementation bugs, documentation drift, proposals and historical evidence; surface contradictions that require a product decision.

Read the relevant architecture change-map row and [documentation maintenance](../../docs/documentation-maintenance.md). Build a compact claim-to-evidence list for material mismatches. Fix clear drift within the requested scope. Ask focused questions about ambiguous intended behavior; continue independent link and naming repairs meanwhile.

Use one canonical source per contract, short indexes and relative links. Keep current product-version literals out of living guides; use Cargo/runtime-derived version displays. Preserve meaningful historical versions, schema/protocol identifiers and compatibility explanations. Product guidance must remain usable within the standalone `app/` distribution; contributor-only material belongs under `dev/`.

Update nearby examples, architecture maps, skill references and the explicit wiki allowlist when relevant. Run `python -B dev/scripts/check_docs.py`, `python -B dev/scripts/build_wiki.py` and affected documentation tests. For changed command examples, exercise a small synthetic case where practical; link checks alone cannot establish semantic accuracy. Mark unverified claims instead of presenting them as tested.

Keep generated wiki pages out of source edits. Wiki publication occurs from a successfully checked `main` revision. Use `apex-merge-dev` or `apex-release` only when integration/publication is requested. For proposed legacy-file removal use [artifact maintenance](../apex-artifacts/SKILL.md) rather than deleting material merely because its name looks old.
