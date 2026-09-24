# APEX benchmark — 24 September 2026

[Download the complete reproduction ZIP](apex-benchmark-2026-09-24.zip).
This is a frozen benchmark of the algorithm dated **24 September 2026**, application commit
[`9bc9dbf`](https://github.com/qunevo/apex/commit/9bc9dbfe9f9d169b1ea1abf1894004384c9be7d7).
APEX development continues to improve quality, performance and modeling coverage.
These results describe the recorded baseline and must not be attributed to a later checkout.
The measured source and harness inside the ZIP, verified by the original manifests, are authoritative.

The ZIP contains all 3,378 original publication files byte-for-byte, plus a third-party dataset notice:
algorithm sources, build assets, dependency locks, input data, all raw runs, schedules,
analysis scripts, tables, figures and provenance. No sibling repository or dataset download is needed.
Python/Rust and their pinned dependencies must be installed separately.

Size: **87.68 MiB** (91,935,272 bytes); about 1.16 GB after extraction.

Archive SHA-256: `a7299c71ac7e523715ae179d0c4078ff6e30cb0f36b3e317b1ae3d4bae27b8d1`

## Scientific scope

The experiment uses **88 public research instances**: 21 job-shop (JSP), 39 flexible
job-shop (FJSP) and 28 permutation flow-shop (PFSP) cases. Sources are
[OR-Library](https://people.brunel.ac.uk/~mastjjb/jeb/orlib/jobshopinfo.html),
[Taillard's instance collection](https://mistic.iict-heig-vd.ch/taillard/problemes.dir/ordonnancement.dir/ordonnancement.html)
and the [SchedulingLab distributions](https://github.com/SchedulingLab/fjsp-instances)
of Brandimarte and Behnke–Geiger. The package preserves 96 original source files,
URLs, SHA-256 fingerprints and all 280 candidate selection decisions.
These instances assume zero job releases, nonpreemptive processing, unit-capacity
machines and no due dates; they do not exercise APEX's full production model.

Eight APEX methods/combinations are compared with pymoo 0.6.2 NSGA-II, SPEA2,
MOEA/D and SMS-EMOA. The protocol uses seeds 19, 42 and 73, at most 1,000 candidate
evaluations per search (one construction for FastPlanner), eight concurrent
processes, one algorithm worker each and a 240-second process watchdog. Objectives
are makespan and total job flowtime. There are no LLM calls.

The 3,168 recorded runs comprise **2,906 valid completions and 262 timeouts**;
independent validation audits 10,522 exported schedules. Quality summaries use
seed medians on the 69 instances completed by every method and seed; coverage
reports retain all 88. Different decoders, host contention and this completion
filter matter when interpreting comparisons. This is descriptive evidence,
not an optimality proof, an equal-wall-time comparison or an independent
state-of-the-art assessment. Gaps are relative to observed best values, not proven optima.

## Extract and reproduce

The original environment was Windows, Python 3.11.9, rustc 1.98.1 and an Intel
Core Ultra 9 275HX (24 logical CPUs). Exact package versions and the host record
are included. Use a fresh external directory and a Python 3.11 environment.
From the repository root, in Bash or Git Bash:

```bash
python -m zipfile -e benchmark/apex-benchmark-2026-09-24.zip ../apex-benchmark-reproduction
cd ../apex-benchmark-reproduction/apex-benchmark-2026-09-24
python -m venv ../venv
if [ -f ../venv/bin/activate ]; then
  source ../venv/bin/activate
else
  source ../venv/Scripts/activate
fi
python -m pip install -r requirements-lock.txt
python -B publication.py validate
python -B reproduce.py --check
```

These checks verify the recorded files, run matrix, objectives and schedules without
running an optimizer. Expect 3,168 records and 10,522 audited schedules.
To regenerate analysis into new output directories:

```bash
python -B export_views.py results/scaling-20260924-current-parallel ../new-views
python -B prepare_figure_data.py results/scaling-20260924-current-parallel ../new-figure-data
python -B plot_figures.py ../new-figure-data
```

For a **new solver experiment**, install Rust and its native linker, then explicitly run:

```bash
python -B reproduce.py --prepare ../new-experiment
python -B reproduce.py --execute ../new-experiment
```

Preparation rebuilds the frozen source, including its recorded supplemental build assets.
Only `--execute` starts the 3,168-run campaign. Preserve the published evidence and compare
new results separately; timings and timeout counts can change with the execution environment.
Dependency installation/build may need network access. CSV/JSON and figures are portable;
the optional Excel renderer uses an additional tool described in the included guide.

## Read and cite the evidence

After extraction, start with `README.md` for the full protocol and `FIGURES.md` for
metric definitions and figure interpretation. `data/README.md` explains selection;
`results/scaling-20260924-current-parallel/` holds the original manifest, recovery record,
raw runs, reports and derived views. `publication_manifest.json` inventories the original
files; the ZIP checksum above also covers the supplemental `THIRD_PARTY_NOTICES.md`.

When citing this package, record **APEX benchmark, 24 September 2026**, the full
algorithm commit `9bc9dbfe9f9d169b1ea1abf1894004384c9be7d7`, experiment ID
`scaling-20260924-current-parallel`, archive checksum and repository revision used.
Also cite the underlying datasets and comparison methods, including:

- Taillard (1993), *Benchmarks for basic scheduling problems*, EJOR 64(2), 278–285; see the author-hosted collection above.
- Brandimarte (1993), [original FJSP paper](https://doi.org/10.1007/BF02023073).
- Behnke and Geiger (2012), [instance research report](https://openhsu.ub.hsu-hh.de/entities/publication/436).
- Blank and Deb (2020), [pymoo: Multi-Objective Optimization in Python](https://doi.org/10.1109/ACCESS.2020.2990567).

The included `LICENSE` governs APEX source and execution. Third-party datasets and
dependencies retain their own terms; the package does not relicense them.
