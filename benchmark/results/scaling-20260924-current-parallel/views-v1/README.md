# APEX benchmark result views

Source run: `scaling-20260924-current-parallel`.

All original run files remain unchanged. XG = FastPlanner; XH = Trainer; XT = tree search; XE = direct genetic algorithm. Sequential combinations are XHT, XHE, XTE and XHTE.

## Files

[Excel workbook](outputs/scaling-20260924/benchmark_views.xlsx) contains the same tables as the CSV/JSON exports. It is a static research snapshot; regenerate it to refresh calculations.

| View | Purpose | Rows |
|---|---|---:|
| [overall.csv](overall.csv) | Equal-instance aggregate, matching the chat table | 12 |
| [by_problem_class.csv](by_problem_class.csv) | JSP / FJSP / PFSP | 36 |
| [by_size.csv](by_size.csv) | Problem class and fixed operation-count band | 96 |
| [by_family.csv](by_family.csv) | Dataset family within each problem class | 60 |
| [by_cohort.csv](by_cohort.csv) | Legacy pilot cases versus newly added cases | 72 |
| [overall_class_balanced.csv](overall_class_balanced.csv) | Equal weight for each problem class | 12 |
| [coverage.csv](coverage.csv) | Successes and timeouts over all 88 instances | 12 |
| [per_instance.csv](per_instance.csv) | Seed medians and min/max spread with sample counts | 1056 |
| [raw_runs.csv](raw_runs.csv) | All individual run metrics; source_json links to full raw evidence | 3168 |

[All tables in JSON](views.json) · [Raw run metrics in JSON](raw_runs.json) · [Provenance and definitions](export_manifest.json)

## Reading the tables

Median of successful seed outcomes within each fully paired instance, then arithmetic mean with equal instance weights. Fully paired means every method and every seed succeeded. Class-balanced view averages the three class means equally.

Null/blank means unavailable. Timeout quality is never imputed as zero. Per-instance medians use available seeds and disclose the valid count; aggregate quality uses fully paired instances only.

Gap/deficit values are percentages: 3.5 means 3.5%, not 350%. Time is measured internal wall seconds. Processing-time MS/FT use the source instances' units, not asserted seconds.

Gaps refer to the best observed value in this experiment, not a proven optimum. Minimum MS and minimum FT can belong to different schedules. Original failure outcomes remain in coverage columns even when excluded from paired quality summaries. No overall superiority claim follows from the successful subset alone.

Eight concurrent single-worker experiments; XG uses one construction, searches up to 1000 evaluations. Runtime and timeout outcomes include shared-host effects.

Post-result descriptive export; class balancing and family views are supplementary perspectives, not independent confirmation.

## Overall, equal instance weights

| Method | Paired instances | HV deficit % | MS gap % | FT gap % | Mean seconds | Timeouts / planned |
|---|---:|---:|---:|---:|---:|---:|
| XG | 69 | 3.498 | 18.405 | 14.587 | 0.035 | 0 / 264 |
| XH | 69 | 0.898 | 3.025 | 4.490 | 39.820 | 46 / 264 |
| XT | 69 | 1.364 | 6.910 | 6.976 | 35.606 | 42 / 264 |
| XE | 69 | 1.700 | 11.226 | 8.841 | 19.784 | 30 / 264 |
| XHT | 69 | 0.728 | 2.537 | 4.001 | 39.819 | 45 / 264 |
| XHE | 69 | 0.466 | 1.844 | 3.053 | 30.068 | 32 / 264 |
| XTE | 69 | 0.879 | 5.417 | 5.345 | 28.241 | 30 / 264 |
| XHTE | 69 | 0.457 | 1.717 | 3.329 | 33.129 | 37 / 264 |
| NSGA-II | 69 | 2.886 | 23.490 | 26.473 | 0.542 | 0 / 264 |
| SPEA2 | 69 | 2.801 | 22.930 | 25.765 | 0.568 | 0 / 264 |
| MOEA/D | 69 | 2.248 | 18.998 | 22.665 | 0.715 | 0 / 264 |
| SMS-EMOA | 69 | 2.902 | 23.444 | 26.439 | 0.543 | 0 / 264 |

## JSP

| Method | Paired instances | HV deficit % | MS gap % | FT gap % | Mean seconds | Timeouts / planned |
|---|---:|---:|---:|---:|---:|---:|
| XG | 19 | 3.560 | 16.837 | 11.323 | 0.022 | 0 / 63 |
| XH | 19 | 0.271 | 1.366 | 1.729 | 26.454 | 3 / 63 |
| XT | 19 | 0.434 | 3.013 | 3.335 | 22.143 | 0 / 63 |
| XE | 19 | 2.309 | 12.461 | 9.965 | 5.580 | 0 / 63 |
| XHT | 19 | 0.192 | 1.193 | 1.145 | 27.275 | 1 / 63 |
| XHE | 19 | 0.331 | 1.516 | 2.116 | 16.561 | 0 / 63 |
| XTE | 19 | 0.490 | 3.279 | 3.736 | 14.613 | 0 / 63 |
| XHTE | 19 | 0.224 | 1.126 | 1.946 | 19.498 | 0 / 63 |
| NSGA-II | 19 | 1.620 | 7.030 | 13.245 | 1.019 | 0 / 63 |
| SPEA2 | 19 | 1.567 | 7.174 | 12.745 | 1.046 | 0 / 63 |
| MOEA/D | 19 | 1.575 | 6.619 | 12.820 | 1.201 | 0 / 63 |
| SMS-EMOA | 19 | 1.606 | 7.160 | 13.029 | 1.021 | 0 / 63 |

## FJSP

| Method | Paired instances | HV deficit % | MS gap % | FT gap % | Mean seconds | Timeouts / planned |
|---|---:|---:|---:|---:|---:|---:|
| XG | 32 | 2.849 | 21.472 | 15.227 | 0.027 | 0 / 117 |
| XH | 32 | 0.171 | 0.907 | 1.829 | 31.733 | 13 / 117 |
| XT | 32 | 1.108 | 7.943 | 6.908 | 27.442 | 12 / 117 |
| XE | 32 | 2.102 | 15.841 | 12.147 | 3.452 | 0 / 117 |
| XHT | 32 | 0.193 | 0.921 | 2.264 | 30.561 | 14 / 117 |
| XHE | 32 | 0.270 | 1.403 | 2.688 | 17.496 | 2 / 117 |
| XTE | 32 | 1.100 | 7.943 | 6.975 | 15.574 | 0 / 117 |
| XHTE | 32 | 0.238 | 1.134 | 2.881 | 21.775 | 7 / 117 |
| NSGA-II | 32 | 4.663 | 44.322 | 47.019 | 0.359 | 0 / 117 |
| SPEA2 | 32 | 4.545 | 43.172 | 45.752 | 0.387 | 0 / 117 |
| MOEA/D | 32 | 3.757 | 36.495 | 40.401 | 0.528 | 0 / 117 |
| SMS-EMOA | 32 | 4.688 | 44.140 | 46.873 | 0.357 | 0 / 117 |

## PFSP

| Method | Paired instances | HV deficit % | MS gap % | FT gap % | Mean seconds | Timeouts / planned |
|---|---:|---:|---:|---:|---:|---:|
| XG | 18 | 4.589 | 14.609 | 16.895 | 0.064 | 0 / 84 |
| XH | 18 | 2.853 | 8.540 | 12.136 | 68.303 | 30 / 84 |
| XT | 18 | 2.799 | 9.188 | 10.940 | 64.331 | 30 / 84 |
| XE | 18 | 0.343 | 1.718 | 1.776 | 63.810 | 30 / 84 |
| XHT | 18 | 2.247 | 6.827 | 10.104 | 69.519 | 30 / 84 |
| XHE | 18 | 0.958 | 2.976 | 4.693 | 66.675 | 30 / 84 |
| XTE | 18 | 0.898 | 3.183 | 4.148 | 65.144 | 30 / 84 |
| XHTE | 18 | 1.091 | 3.376 | 5.586 | 67.703 | 30 / 84 |
| NSGA-II | 18 | 1.064 | 3.832 | 3.910 | 0.363 | 0 / 84 |
| SPEA2 | 18 | 1.002 | 3.576 | 3.973 | 0.384 | 0 / 84 |
| MOEA/D | 18 | 0.276 | 0.957 | 1.529 | 0.535 | 0 / 84 |
| SMS-EMOA | 18 | 1.094 | 3.839 | 4.267 | 0.368 | 0 / 84 |

## Overall, equal problem-class weights

| Method | Paired instances | HV deficit % | MS gap % | FT gap % | Mean seconds | Timeouts / planned |
|---|---:|---:|---:|---:|---:|---:|
| XG | — | 3.666 | 17.639 | 14.482 | 0.037 | — / — |
| XH | — | 1.098 | 3.604 | 5.231 | 42.164 | — / — |
| XT | — | 1.447 | 6.714 | 7.061 | 37.972 | — / — |
| XE | — | 1.585 | 10.007 | 7.963 | 24.281 | — / — |
| XHT | — | 0.877 | 2.981 | 4.504 | 42.452 | — / — |
| XHE | — | 0.520 | 1.965 | 3.165 | 33.577 | — / — |
| XTE | — | 0.829 | 4.802 | 4.953 | 31.777 | — / — |
| XHTE | — | 0.518 | 1.879 | 3.471 | 36.325 | — / — |
| NSGA-II | — | 2.449 | 18.395 | 21.391 | 0.580 | — / — |
| SPEA2 | — | 2.371 | 17.974 | 20.824 | 0.606 | — / — |
| MOEA/D | — | 1.869 | 14.690 | 18.250 | 0.755 | — / — |
| SMS-EMOA | — | 2.463 | 18.380 | 21.389 | 0.582 | — / — |

