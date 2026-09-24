# Expanded APEX MS / job-flowtime benchmark

Run: `scaling-20260924-current-parallel`. Recorded 3168 of 3168 planned runs.

Statuses: {'completed': 2906, 'timeout': 262}. Fully paired instances: 69 of 88.

This is a follow-up stress test informed by earlier development, not independent confirmation. Every original pilot case is retained and freshly evaluated. Selection was fixed before this run; see `protocol.json` and all 280 inclusion decisions in `selection.json`.

Each search receives 1,000 evaluations with a common 240-second process watchdog. F uses one construction. Timeout rows have no inferred quality. Three seeds support descriptive comparisons only. Internal seconds exclude imports and output/audit; the watchdog includes them. This is not an equal-time comparison.

Execution uses 8 concurrent local runs, with one worker per algorithm. Resource contention and CPU heterogeneity affect runtime and timeout outcomes. These timings do not establish isolated performance or speedups. The run requires no LLM/API calls or agent monitoring.

Primary quality summaries below use only instances completed validly by every method for every seed. First take the median across seeds within each instance, then average equally across instances. Missing coverage can make this subset unrepresentative; do not infer an overall winner. Small: <=150 operations; medium: 151–500; large: >500. Per-instance conditional summaries are exported in `results.json`.

Gaps refer to best observed values across this run, not optima. MS and FT extrema may describe different schedules. Large-case results at this modest search budget do not establish convergence or industrial deployment performance.

| Class | Operations | Method | Valid / planned | Timeouts | Errors | Paired instances | HV deficit % | MS gap % | FT gap % | Paired seconds |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| FJSP | medium | F | 60/60 | 0 | 0 | 13 | 2.987 | 22.097 | 17.464 | 0.045 |
| FJSP | medium | T | 47/60 | 13 | 0 | 13 | 0.120 | 0.593 | 1.632 | 56.086 |
| FJSP | medium | B | 48/60 | 12 | 0 | 13 | 1.416 | 10.650 | 8.617 | 48.335 |
| FJSP | medium | G | 60/60 | 0 | 0 | 13 | 2.453 | 18.563 | 15.126 | 5.398 |
| FJSP | medium | T-B | 46/60 | 14 | 0 | 13 | 0.148 | 0.849 | 1.995 | 54.502 |
| FJSP | medium | T-G | 58/60 | 2 | 0 | 13 | 0.209 | 1.098 | 2.558 | 30.997 |
| FJSP | medium | B-G | 60/60 | 0 | 0 | 13 | 1.405 | 10.733 | 8.703 | 27.015 |
| FJSP | medium | T-B-G | 53/60 | 7 | 0 | 13 | 0.214 | 0.904 | 3.047 | 38.885 |
| FJSP | medium | NSGA-II | 60/60 | 0 | 0 | 13 | 3.929 | 41.225 | 47.397 | 0.578 |
| FJSP | medium | SPEA2 | 60/60 | 0 | 0 | 13 | 3.886 | 40.875 | 46.353 | 0.612 |
| FJSP | medium | MOEA-D | 60/60 | 0 | 0 | 13 | 3.261 | 35.383 | 42.168 | 0.754 |
| FJSP | medium | SMS-EMOA | 60/60 | 0 | 0 | 13 | 3.905 | 40.899 | 46.825 | 0.576 |
| FJSP | small | F | 57/57 | 0 | 0 | 19 | 2.754 | 21.045 | 13.696 | 0.015 |
| FJSP | small | T | 57/57 | 0 | 0 | 19 | 0.205 | 1.122 | 1.964 | 15.071 |
| FJSP | small | B | 57/57 | 0 | 0 | 19 | 0.898 | 6.090 | 5.739 | 13.148 |
| FJSP | small | G | 57/57 | 0 | 0 | 19 | 1.863 | 13.978 | 10.109 | 2.121 |
| FJSP | small | T-B | 57/57 | 0 | 0 | 19 | 0.223 | 0.971 | 2.448 | 14.181 |
| FJSP | small | T-G | 57/57 | 0 | 0 | 19 | 0.311 | 1.611 | 2.776 | 8.259 |
| FJSP | small | B-G | 57/57 | 0 | 0 | 19 | 0.891 | 6.033 | 5.792 | 7.747 |
| FJSP | small | T-B-G | 57/57 | 0 | 0 | 19 | 0.254 | 1.291 | 2.767 | 10.067 |
| FJSP | small | NSGA-II | 57/57 | 0 | 0 | 19 | 5.166 | 46.440 | 46.760 | 0.210 |
| FJSP | small | SPEA2 | 57/57 | 0 | 0 | 19 | 4.997 | 44.743 | 45.342 | 0.234 |
| FJSP | small | MOEA-D | 57/57 | 0 | 0 | 19 | 4.097 | 37.256 | 39.191 | 0.373 |
| FJSP | small | SMS-EMOA | 57/57 | 0 | 0 | 19 | 5.223 | 46.357 | 46.905 | 0.206 |
| JSP | large | F | 24/24 | 0 | 0 | 6 | 1.415 | 10.713 | 13.129 | 0.049 |
| JSP | large | T | 21/24 | 3 | 0 | 6 | 0.054 | 0.766 | 0.939 | 61.249 |
| JSP | large | B | 24/24 | 0 | 0 | 6 | 0.588 | 3.714 | 6.846 | 51.492 |
| JSP | large | G | 24/24 | 0 | 0 | 6 | 1.383 | 10.360 | 13.005 | 11.090 |
| JSP | large | T-B | 23/24 | 1 | 0 | 6 | 0.095 | 0.758 | 1.086 | 64.294 |
| JSP | large | T-G | 24/24 | 0 | 0 | 6 | 0.130 | 1.013 | 1.514 | 37.881 |
| JSP | large | B-G | 24/24 | 0 | 0 | 6 | 0.630 | 3.937 | 7.375 | 33.236 |
| JSP | large | T-B-G | 24/24 | 0 | 0 | 6 | 0.186 | 0.968 | 2.603 | 45.242 |
| JSP | large | NSGA-II | 24/24 | 0 | 0 | 6 | 1.614 | 8.699 | 19.351 | 2.207 |
| JSP | large | SPEA2 | 24/24 | 0 | 0 | 6 | 1.610 | 8.852 | 19.137 | 2.220 |
| JSP | large | MOEA-D | 24/24 | 0 | 0 | 6 | 1.590 | 8.820 | 18.787 | 2.394 |
| JSP | large | SMS-EMOA | 24/24 | 0 | 0 | 6 | 1.655 | 9.086 | 19.641 | 2.186 |
| JSP | medium | F | 24/24 | 0 | 0 | 8 | 2.313 | 16.124 | 10.054 | 0.013 |
| JSP | medium | T | 24/24 | 0 | 0 | 8 | 0.250 | 2.095 | 1.592 | 15.760 |
| JSP | medium | B | 24/24 | 0 | 0 | 8 | 0.443 | 3.722 | 2.214 | 13.035 |
| JSP | medium | G | 24/24 | 0 | 0 | 8 | 2.065 | 14.074 | 9.641 | 4.410 |
| JSP | medium | T-B | 24/24 | 0 | 0 | 8 | 0.151 | 1.644 | 1.011 | 15.476 |
| JSP | medium | T-G | 24/24 | 0 | 0 | 8 | 0.278 | 2.046 | 1.889 | 10.087 |
| JSP | medium | B-G | 24/24 | 0 | 0 | 8 | 0.506 | 4.094 | 2.704 | 9.031 |
| JSP | medium | T-B-G | 24/24 | 0 | 0 | 8 | 0.201 | 1.307 | 1.635 | 11.464 |
| JSP | medium | NSGA-II | 24/24 | 0 | 0 | 8 | 1.818 | 9.132 | 12.923 | 0.670 |
| JSP | medium | SPEA2 | 24/24 | 0 | 0 | 8 | 1.773 | 9.073 | 12.540 | 0.708 |
| JSP | medium | MOEA-D | 24/24 | 0 | 0 | 8 | 1.691 | 8.162 | 11.877 | 0.866 |
| JSP | medium | SMS-EMOA | 24/24 | 0 | 0 | 8 | 1.760 | 9.128 | 12.382 | 0.689 |
| JSP | small | F | 15/15 | 0 | 0 | 5 | 8.127 | 25.327 | 11.186 | 0.002 |
| JSP | small | T | 15/15 | 0 | 0 | 5 | 0.565 | 0.919 | 2.897 | 1.811 |
| JSP | small | B | 15/15 | 0 | 0 | 5 | 0.234 | 1.037 | 0.913 | 1.497 |
| JSP | small | G | 15/15 | 0 | 0 | 5 | 3.811 | 12.401 | 6.837 | 0.842 |
| JSP | small | T-B | 15/15 | 0 | 0 | 5 | 0.371 | 0.994 | 1.432 | 1.729 |
| JSP | small | T-G | 15/15 | 0 | 0 | 5 | 0.658 | 1.270 | 3.202 | 1.334 |
| JSP | small | B-G | 15/15 | 0 | 0 | 5 | 0.297 | 1.186 | 1.021 | 1.198 |
| JSP | small | T-B-G | 15/15 | 0 | 0 | 5 | 0.307 | 1.025 | 1.654 | 1.463 |
| JSP | small | NSGA-II | 15/15 | 0 | 0 | 5 | 1.310 | 1.662 | 6.433 | 0.152 |
| JSP | small | SPEA2 | 15/15 | 0 | 0 | 5 | 1.186 | 2.121 | 5.404 | 0.179 |
| JSP | small | MOEA-D | 15/15 | 0 | 0 | 5 | 1.371 | 1.508 | 7.168 | 0.306 |
| JSP | small | SMS-EMOA | 15/15 | 0 | 0 | 5 | 1.303 | 1.701 | 6.128 | 0.155 |
| PFSP | large | F | 36/36 | 0 | 0 | 2 | 2.049 | 18.057 | 12.246 | 0.186 |
| PFSP | large | T | 6/36 | 30 | 0 | 2 | 1.189 | 9.908 | 8.174 | 210.927 |
| PFSP | large | B | 6/36 | 30 | 0 | 2 | 1.322 | 11.643 | 7.953 | 196.931 |
| PFSP | large | G | 6/36 | 30 | 0 | 2 | 0.329 | 2.692 | 2.768 | 199.709 |
| PFSP | large | T-B | 6/36 | 30 | 0 | 2 | 1.132 | 9.511 | 8.079 | 223.937 |
| PFSP | large | T-G | 6/36 | 30 | 0 | 2 | 0.633 | 4.560 | 5.752 | 203.923 |
| PFSP | large | B-G | 6/36 | 30 | 0 | 2 | 0.493 | 3.943 | 4.577 | 195.951 |
| PFSP | large | T-B-G | 6/36 | 30 | 0 | 2 | 0.726 | 5.169 | 6.282 | 214.075 |
| PFSP | large | NSGA-II | 36/36 | 0 | 0 | 2 | 0.526 | 4.681 | 3.108 | 1.035 |
| PFSP | large | SPEA2 | 36/36 | 0 | 0 | 2 | 0.479 | 4.972 | 2.575 | 1.060 |
| PFSP | large | MOEA-D | 36/36 | 0 | 0 | 2 | 0.151 | 1.554 | 0.574 | 1.274 |
| PFSP | large | SMS-EMOA | 36/36 | 0 | 0 | 2 | 0.516 | 5.129 | 3.591 | 1.141 |
| PFSP | medium | F | 30/30 | 0 | 0 | 10 | 4.286 | 15.081 | 17.583 | 0.071 |
| PFSP | medium | T | 30/30 | 0 | 0 | 10 | 2.755 | 9.038 | 13.223 | 74.635 |
| PFSP | medium | B | 30/30 | 0 | 0 | 10 | 3.053 | 10.694 | 12.451 | 70.509 |
| PFSP | medium | G | 30/30 | 0 | 0 | 10 | 0.348 | 1.705 | 1.858 | 69.244 |
| PFSP | medium | T-B | 30/30 | 0 | 0 | 10 | 2.402 | 7.568 | 11.766 | 74.288 |
| PFSP | medium | T-G | 30/30 | 0 | 0 | 10 | 0.989 | 3.102 | 5.224 | 73.437 |
| PFSP | medium | B-G | 30/30 | 0 | 0 | 10 | 0.954 | 3.269 | 5.015 | 72.442 |
| PFSP | medium | T-B-G | 30/30 | 0 | 0 | 10 | 1.270 | 3.737 | 6.725 | 73.262 |
| PFSP | medium | NSGA-II | 30/30 | 0 | 0 | 10 | 0.930 | 3.650 | 3.923 | 0.367 |
| PFSP | medium | SPEA2 | 30/30 | 0 | 0 | 10 | 0.928 | 3.531 | 4.121 | 0.387 |
| PFSP | medium | MOEA-D | 30/30 | 0 | 0 | 10 | 0.282 | 0.709 | 1.751 | 0.539 |
| PFSP | medium | SMS-EMOA | 30/30 | 0 | 0 | 10 | 1.017 | 3.749 | 4.414 | 0.355 |
| PFSP | small | F | 18/18 | 0 | 0 | 6 | 5.940 | 12.671 | 17.298 | 0.011 |
| PFSP | small | T | 18/18 | 0 | 0 | 6 | 3.571 | 7.255 | 11.645 | 10.209 |
| PFSP | small | B | 18/18 | 0 | 0 | 6 | 2.868 | 5.860 | 9.418 | 9.833 |
| PFSP | small | G | 18/18 | 0 | 0 | 6 | 0.338 | 1.416 | 1.309 | 9.452 |
| PFSP | small | T-B | 18/18 | 0 | 0 | 6 | 2.360 | 4.698 | 8.009 | 10.096 |
| PFSP | small | T-G | 18/18 | 0 | 0 | 6 | 1.014 | 2.237 | 3.453 | 9.654 |
| PFSP | small | B-G | 18/18 | 0 | 0 | 6 | 0.939 | 2.787 | 2.560 | 9.379 |
| PFSP | small | T-B-G | 18/18 | 0 | 0 | 6 | 0.913 | 2.178 | 3.456 | 9.648 |
| PFSP | small | NSGA-II | 18/18 | 0 | 0 | 6 | 1.466 | 3.853 | 4.154 | 0.131 |
| PFSP | small | SPEA2 | 18/18 | 0 | 0 | 6 | 1.299 | 3.187 | 4.193 | 0.153 |
| PFSP | small | MOEA-D | 18/18 | 0 | 0 | 6 | 0.306 | 1.172 | 1.477 | 0.282 |
| PFSP | small | SMS-EMOA | 18/18 | 0 | 0 | 6 | 1.415 | 3.559 | 4.246 | 0.131 |

## Dataset provenance

JSP legacy: Fisher–Thompson, Lawrence and Adams–Balas–Zawack via [OR-Library](https://people.brunel.ac.uk/~mastjjb/jeb/orlib/jobshopinfo.html). JSP and PFSP size blocks: [Taillard (1993), author-hosted data](https://mistic.iict-heig-vd.ch/taillard/problemes.dir/ordonnancement.dir/ordonnancement.html). FJSP: Brandimarte (1993) and Behnke–Geiger (2012) via the [SchedulingLab distribution](https://github.com/SchedulingLab/fjsp-instances). Source bytes and SHA-256 hashes are retained; the mirror's bounds are not used as reference optima.
