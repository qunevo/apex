# APEX benchmark: reproduction code and current results

This directory contains the reproduction code and current numerical-core
results. No experiment starts automatically. Historical runs, smoke tests,
temporary requests, environments and build caches are archived outside the
publication tree.

The complete `benchmark` directory can be copied out of the repository and
used on its own. It includes the selected instances, all original dataset files,
the selection ledger, a ZIP of the measured algorithm, analysis code and current results.
Python/Rust and the pinned dependencies must be installed; no dataset downloads
or sibling APEX/paper checkout are required for the supported reproduction path.

**Algorithm baseline:** 24 September 2026, application commit
[9bc9dbf](https://github.com/qunevo/apex/commit/9bc9dbfe9f9d169b1ea1abf1894004384c9be7d7).
The run's hashed source snapshot is authoritative; later application code must
not be substituted. See [baseline scope](ALGORITHM_BASELINE.md).

## Results and figures

| Artifact | Contents |
|---|---|
| [Instance data](data/README.md) | 88 selected instances, all 280 selection decisions and 96 original source files, included in Git |
| [Algorithm source ZIP](algorithm-9bc9dbf.zip) | The measured source, original harness and required build assets; extracted only outside the repository |
| [Result views](results/scaling-20260924-current-parallel/views-v1/README.md) | Raw, class and overall CSV/JSON tables and Excel; additional size, family, cohort, completion and seed views |
| [Figure guide](FIGURES.md) | Recommended paper figures, input tables and statistical definitions |
| [Prepared figure data](results/scaling-20260924-current-parallel/figures-v1/manifest.json) | Boxplot statistics, variance, ECDFs, paired differences, scaling, archives and convergence |
| [Figure candidates](results/scaling-20260924-current-parallel/figures-v1/plots/) | Seven figures in vector PDF/SVG and preview PNG formats |
| [Full raw evidence](results/scaling-20260924-current-parallel/runs/) | All 3,168 original run JSONs, including failures, traces and schedules |
| [Selection](results/scaling-20260924-current-parallel/selection.json) | All 280 inclusion decisions, source URLs and hashes |
| [Original manifest](results/scaling-20260924-current-parallel/manifest.json) | Settings, environment, fingerprints and original method IDs |
| [Publication inventory](publication_manifest.json) | SHA-256 inventory, explicitly archived executable and supplemental build-resource records |
| [Validation](publication_validation.json) | File integrity, complete run matrix and independent schedule audit |

The large raw-evidence directory contains current results, not temporary files.
Every original run record remains byte-for-byte unchanged.
The local `.gitattributes` disables line-ending conversion so Git checkout
preserves all recorded SHA-256 fingerprints on Windows and other platforms.

## Experiment and interpretation

There are 88 instances: JSP 21, FJSP 39, PFSP 28. All 15 pilot cases were
freshly rerun. The sample includes all 15 distributed Brandimarte instances
and two SHA-256-ranked identifiers per declared Taillard and Behnke-Geiger
size block. Historical measurements are not mixed into this experiment.
This is descriptive stress-test evidence following development work, not
independent confirmation or a state-of-the-art claim.

Methods: **XG** FastPlanner, **XH** Trainer, **XT** tree search, **XE** GA;
sequential combinations **XHT**, **XHE**, **XTE**, **XHTE**. Original IDs remain
in raw files, with the public-name mapping in method_names.json. Baselines
are pymoo 0.6.2 NSGA-II, SPEA2, MOEA/D and SMS-EMOA with recorded discrete
adapters. External JSP/FJSP decoding uses earliest feasible machine-gap
insertion; PFSP uses a common permutation. APEX uses its validated constructor.
Complete implementations, including different decoder costs, are compared.

Searches receive at most 1,000 candidate evaluations; XG makes one construction.
Pairs split the budget equally, the triple into thirds. Unused evaluations carry
forward; hybrids hand over one validated incumbent. Internal and external
population settings retain the recorded defaults, without new tuning.
Seeds are 19, 42 and 73. Eight concurrent local processes use one algorithm
worker each and a common 240-second process watchdog. There are no LLM calls.
Exact operators, phase settings and counters remain in raw JSON and frozen code.

The matrix has 2,906 valid completions and 262 timeouts. Quality summaries take
each instance's seed median, then average equally over the 69 instances valid
for every method and seed. Coverage retains all 88 instances. The successful
subset alone does not establish an overall reliability-adjusted winner.
Equal-class weighting is a supplementary post-result view.

Objectives are makespan (MS) and total job flowtime (FT). Hypervolume uses
(MS/S, FT/(n*S)), reference (1.1, 1.1), where S sums maximum eligible operation
durations and n counts jobs. Gaps refer to the best observed instance value,
not an optimum. Minimum MS and FT may describe different schedules.
Internal runtime includes construction, search, observation and online
validation; it excludes imports, input parsing, final export and post-run audit.
The watchdog includes process overhead. Host contention remains part of the
measurements: this is not an equal-wall-time or isolated-speed comparison.

## Reproduce analysis without executing an optimiser

The original environment used Python 3.11.9 and rustc 1.98.1 on Windows.
Dependencies are pinned in requirements-lock.txt and the frozen Cargo locks.
Create a Python environment outside this directory, install the pinned
requirements and activate it. Run every command below from this `benchmark`
directory, including when it is a standalone copy:

```sh
python -m pip install -r requirements-lock.txt
python -B publication.py validate
python -B reproduce.py --check
python -B export_views.py results/scaling-20260924-current-parallel /path/to/new-views
python -B prepare_figure_data.py results/scaling-20260924-current-parallel /path/to/new-figure-data
python -B plot_figures.py /path/to/new-figure-data
python -B -m pytest test_export_views.py test_figure_data.py test_publication.py -q -p no:cacheprovider
```

The validator checks every retained frozen source/input fingerprint, all raw
run fingerprints, the complete matrix and exported schedules/objectives.
It does not require the original Windows executable and never invokes a solver.
The original integrity report and completion-recovery record remain unchanged.

CSV/JSON and vector figures are the portable outputs. The optional Excel
renderer uses @oai/artifact-tool and consumes views.json; the existing workbook
was independently checked cell-by-cell against that JSON.

## Explicitly rerun the frozen experiment

Preparation and execution are separate. Use a new directory outside this tree.
Preparation builds the recorded algorithm; only execution starts experiments:

```sh
python -B reproduce.py --prepare /path/to/new-reproduction
python -B reproduce.py --execute /path/to/new-reproduction
```

Preparation extracts `algorithm-9bc9dbf.zip` into the new external directory
and copies exact inputs. The archive contains the algorithm actually measured
at the documented baseline, not a later development version. No legacy source
tree is kept unpacked in this repository. Two compile-time resources for unused
service/UI modules were missing from the original source snapshot. Copies from
the documented baseline commit are identified in `reproduction-assets/manifest.json`
inside the ZIP.
No frozen scheduling module is changed. The original host executable is archived;
its original hash is retained. A rebuilt binary receives its own run manifest.

Use `reproduce.py` for the published experiment. The top-level `run.py` and
`instances.py` retain historical pilot helpers; they are not the entry point for
this 88-instance campaign. The ZIP used by `reproduce.py` includes the complete
measured Rust core and does not depend on `../src/rust`.

The original worker/coordinator is retained. The original campaign recovered
from a Windows status-file replacement failure after queued runs had finished;
see completion-recovery.json. If that condition recurs in a future run, inspect
the complete raw matrix and use that run's frozen scaling_report.py with
--validate to regenerate only its report. Never call a partial run complete.
No monitoring agent or recurring subscription is installed.

## Provenance

JSP sources include OR-Library Fisher-Thompson, Lawrence and Adams-Balas-Zawack,
plus Taillard's author-hosted JSP files. PFSP uses Taillard's author-hosted data.
FJSP uses SchedulingLab distributions of Brandimarte and Behnke-Geiger.
Selection metadata contains URLs and hashes; original bytes are retained.
Cite datasets and algorithms when using their results. The APEX source grant is
the included [LICENSE](LICENSE), copied unchanged from the repository root;
no different license is granted here to third-party data
or dependencies.
