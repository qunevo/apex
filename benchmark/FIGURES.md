# Paper figure data

Current prepared data: `results/scaling-20260924-current-parallel/figures-v1/`.
The recorded algorithms are XG, XH, XT, XE, XHT, XHE, XTE and XHTE, plus the
four external baselines. All views are descriptive and were chosen after seeing
the completed experiment. No new solver run is needed to recreate them.

## Recommended manuscript selection

| Figure | Scientific purpose | Prepared data |
|---|---|---|
| Boxplots with individual points, faceted by problem class | Distribution across instances; reveal skew, outliers and heterogeneous performance | `boxplot_points.csv`, `boxplot_stats.csv`, `boxplot_outliers.csv` |
| Completion heatmap by class and size | Make failures on large inputs visible alongside successful-case quality | `coverage_heatmap.csv` |
| Quality versus internal runtime, with logarithmic time axis | Show the measured quality/computation tradeoff | `quality_time.csv` |
| Convergence over evaluations; optionally over time | Show progress, plateaus and the effect of stopping | `convergence_runs.csv`, `convergence_summary.csv` |
| ECDF of relative gap/deficit | Show the fraction of instances reaching each quality threshold | `ecdf.csv` |
| Paired method differences or win/tie/loss heatmap | Compare methods on the same instances, without comparing unrelated means | `paired_differences.csv`, `paired_comparison_summary.csv` |
| Runtime and quality versus operation count | Examine size trends, retaining the instance and completion indicators | `scaling_points.csv` |
| Objective-space Pareto archives for disclosed example instances | Show makespan/flowtime tradeoffs within an instance | `archive_points.csv` |
| Three seed points or min/max spans per instance | Show stochastic repeatability separately from instance heterogeneity | `seed_statistics.csv`, `seed_variation_summary.csv`, `../views-v1/raw_runs.csv` |

Use the class-level boxplots, completion heatmap and convergence figure as
the main figures. Add the quality/time plot if the implementation-time tradeoff
is central. ECDFs and paired differences are useful supplementary views; avoid
repeating every view of the same metric in the main text.

## Statistical definitions and population

The primary boxplot population contains one median over three seeds for each
fully paired instance: every method must have succeeded for every seed. The
overall plot therefore describes 69 instances; the class plots contain JSP 19,
FJSP 32 and PFSP 18. This is variation across instances, not a sampling-error
estimate of the mean. Full completion denominators retain all 88 instances and
all 3,168 planned runs, including all 262 timeouts.

Quantiles use linear interpolation (Hyndman-Fan type 7). Whiskers end at the
most extreme observed values within 1.5 IQR; fences, actual min/max and each
outlier are exported separately. Outliers are retained in the analysis and
drawn as individual points. Zero-IQR and constant samples remain present.
Sample variance and standard deviation use n-1 and are unavailable for n<2.
Empty samples stay blank. The raw point table permits different rendering
conventions without reconstructing the original experiment.

Seed statistics operate within one instance and one method, separately from
the cross-instance boxplots. They disclose the number of successful seeds.
Three seeds support a dot plot or range, but not a convincing density estimate
or a precise confidence interval. XG quality repetitions are deterministic;
runtime can still vary. No significance tests or confidence intervals are
introduced by this export. All pairwise contrasts are retained, with A minus B
and a numerical tie tolerance of 1e-9.

Gaps and deficits are percentages relative to the best observed value in this
experiment, not known optimality gaps. Makespan and job-flowtime raw values have
source processing-time units. The quality/time and convergence-time views use
measured internal seconds on the concurrent host; they do not establish
isolated runtimes or results of an equal-time rerun. XG uses one construction.

## Traces and missingness

Every saved successful trace is replayed as an observed nondominated prefix.
The final reconstructed HV, makespan and flowtime extrema must agree with the
saved result. A checkpoint before a feasible solution has missing quality.
A completed method carries its final solution forward; this does not invent
additional evaluations. Timeouts have no saved audited trace and are never
assigned an inferred objective value. Per-checkpoint summaries require all
methods and all seeds to have an observed feasible solution for an instance.
The common sample size is explicit and can change across checkpoints.

The supplied convergence figure displays only checkpoints at which the entire
terminal paired population is available, preserving a fixed sample along the
displayed curves. The full checkpoint data remain available. For a later time
plot, either adopt a fixed shared sample or display changing support explicitly.
No time-to-target survival curve is supplied because timeout traces are absent.

`archive_points.csv` contains every exported archive point and identifies the
original run. Draw fronts per instance; pooling raw objective coordinates from
different instances is not a Pareto front. Example selection must be based on
a disclosed rule independent of the plotted performance, or explicitly called
illustrative.

## Reproduction

```sh
python benchmark/prepare_figure_data.py benchmark/results/scaling-20260924-current-parallel /path/to/new-figure-data
python benchmark/plot_figures.py /path/to/new-figure-data
```

Preparation uses only the Python standard library and the local export module.
Rendering needs Matplotlib and NumPy; versions are pinned in
`requirements-lock.txt`. PDF and SVG are vector outputs for the manuscript;
PNG files are previews. `manifest.json` records definitions and source hashes,
and `raw_source_hashes.json` covers every original raw run. These are analysis
commands, not experiment launchers.
