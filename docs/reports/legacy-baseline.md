# Legacy reference at the start of the Rust implementation

## Source removal

On 24 September 2026, the v2 Python/Cython implementation was removed at the user's explicit request, together with its Docker files, Python bootstrap, synthetic integration test and three legacy comparison scripts. Rust is the sole scheduling implementation in this source tree. Historical reports and the source hash manifest remain as evidence of earlier reviews and measurements; paths to v2 in those records identify removed files.

Direct v2 execution comparisons can no longer be rerun from this checkout. The Rust semantic, migration and regression tests remain runnable. Removing the legacy source does not establish complete feature parity, equal solution quality or production readiness.

## Historical baseline

The workspace had no `.git` directory. At the start of the Rust implementation, the legacy implementation was retained under `src/v2`. [The source manifest](legacy-baseline.json) records hashes of the former reference Python/Cython source files, excluding private customization directories and binary artifacts.

The standard path is Python orchestration and Cython fast decoding. Fastplanner constructs a dispatch sequence, trainer tunes queue parameters, and optional refinement/deep search also use fast decoding. CP-SAT code and an unconditional OR-Tools dependency exist but are not selected by the reviewed standard pipeline; result transfer in that path is incomplete.

The reusable domain concepts include jobs/operations, alternative resource lists, conditional pre/post tasks, shift capacity/productivity, dependency graphs, previous plans, frozen horizons and fixed resource sequences. The [operational contract](../architecture/core-semantics.md) cites their source locations and the observed limits.

Migration corrections include explicit missing-work errors, explicit outside-calendar behavior, separate work segments and reservations, independent checking of final assignments and precise lock dimensions. The new implementation does not claim binary compatibility or complete behavioral equivalence with v2. The generic legacy demo was used for historical comparisons and has now been removed with its Python/Cython build artifacts.

The earlier publication audit identified residual binary/archive artifacts in the workspace. This implementation makes no new publication-readiness claim and does not publish the repository. Keep using the separate publication audit before distribution.
