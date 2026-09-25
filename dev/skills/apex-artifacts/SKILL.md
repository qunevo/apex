---
name: apex-artifacts
description: Audit APEX generated and legacy artifacts, distinguish reproducible output from required source or evidence, and perform scoped cleanup with a reviewable removal list.
---

# Maintain repository artifacts

Read [source publication](../../docs/publication.md) and [repository layout](../../docs/repository-layout.md). Inventory only the requested area. Identify each candidate's producer, consumers/references, tracked status, reproducibility and reason to retain it. Age, a legacy filename or lack of an obvious import does not establish that a file is disposable.

Present exact candidate paths and classify them as required source, generated output, historical evidence, local runtime/customer data or unresolved. Keep frozen `benchmark/` evidence, the migration audit, license documents, schemas needed by consumers and lockfiles unless a specific authorized change replaces them. Local `.apex` stores, credentials and customer work are not routine repository-cleanup targets.

For a general audit or uncertain material, propose the removal/move list and wait for its approval. A prior explicit cleanup request covering identified reproducible outputs already authorizes that scope; do not add a redundant gate. Preserve unknown or unique evidence and ask why it exists before removing it. Do not rewrite Git history as incidental cleanup.

Before any recursive removal resolve each path, check that it stays within the approved directory and reject symlinks or traversal outside it. Use one shell end-to-end and literal paths. Prefer selective removal of known outputs to broad extension-based deletion. Update producers, consumers, links and ignore rules together; consolidate useful historical findings in the migration audit instead of multiplying archives.

Run the publication scan on a clean source export, relevant documentation checks and standalone verification if distribution contents changed. Explain that the scanner checks common leaks/artifacts, not synthetic-data provenance or Git history. Report exact removed/retained groups and unresolved ownership. Never claim the repository is clean based only on its current tracked file list.
