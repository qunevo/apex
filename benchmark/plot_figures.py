"""Render manuscript candidates from the prepared CSV tables, without solvers."""
import argparse
import csv
import json
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.lines import Line2D
import numpy as np

from export_views import NAMES


def render(directory):
    def read(name):
        with (directory / f"{name}.csv").open(encoding="utf-8-sig", newline="") as f:
            return list(csv.DictReader(f))

    methods = list(NAMES.values())
    classes = ["JSP", "FJSP", "PFSP"]
    colours = dict(zip(methods, plt.get_cmap("tab20").colors[::1][:len(methods)]))
    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 9, "axes.spines.top": False,
                         "axes.spines.right": False, "pdf.fonttype": 42, "svg.fonttype": "none"})
    outputs = directory / "plots"
    outputs.mkdir(exist_ok=True)
    saved = []

    def save(fig, name):
        for extension in ["pdf", "svg", "png"]:
            fig.savefig(outputs / f"{name}.{extension}", dpi=180, bbox_inches="tight")
        plt.close(fig)
        saved.append(name)

    stats, points = read("boxplot_stats"), read("boxplot_points")
    for metric, label, name in [("hv_deficit_pct", "HV deficit (%)", "quality_boxplots"), ("makespan_gap_pct", "Makespan gap (%)", "makespan_boxplots"), ("flowtime_gap_pct", "Job-flowtime gap (%)", "flowtime_boxplots")]:
        fig, axes = plt.subplots(1, 3, figsize=(10.5, 5), sharey=True, layout="constrained")
        for axis, kind in zip(axes, classes):
            for j, method in enumerate(methods):
                row = next(r for r in stats if r["group_type"] == "class" and r["problem_class"] == kind and r["method"] == method and r["metric"] == metric)
                box = dict(med=float(row["median"]), q1=float(row["q1"]), q3=float(row["q3"]), whislo=float(row["whisker_low"]), whishi=float(row["whisker_high"]), fliers=[])
                axis.bxp([box], positions=[j], widths=.56, orientation="horizontal", showfliers=False,
                         patch_artist=True, boxprops={"facecolor": colours[method], "alpha": .32}, medianprops={"color": "black"})
                values = [float(r["value"]) for r in points if r["problem_class"] == kind and r["method"] == method and r["metric"] == metric]
                jitter = np.linspace(-.15, .15, len(values))
                axis.scatter(values, j + jitter, s=7, color=colours[method], edgecolor="none", alpha=.65, zorder=3)
            axis.set_title(f"{kind} (n={len(values)} instances)")
            axis.set_xlabel(label)
            axis.set_yticks(range(len(methods)), methods)
            axis.grid(axis="x", color="#dddddd", linewidth=.5)
        axes[0].invert_yaxis()
        fig.suptitle("Variation across instances · one median over three seeds per instance", fontsize=11)
        save(fig, name)

    coverage = read("coverage_heatmap")
    groups = [(kind, band) for kind in classes for band in ["1-150", "151-500", "501+"] if any(r["problem_class"] == kind and r["operation_band"] == band for r in coverage)]
    array = np.array([[float(next(r["success_pct"] for r in coverage if r["method"] == method and (r["problem_class"], r["operation_band"]) == group)) for group in groups] for method in methods])
    fig, axis = plt.subplots(figsize=(9, 5.6), layout="constrained")
    plot = axis.imshow(array, vmin=0, vmax=100, cmap="YlGnBu", aspect="auto")
    axis.set_xticks(range(len(groups)), [f"{kind}\n{band}" for kind, band in groups])
    axis.set_yticks(range(len(methods)), methods)
    for i in range(len(methods)):
        for j in range(len(groups)):
            row = next(r for r in coverage if r["method"] == methods[i] and (r["problem_class"], r["operation_band"]) == groups[j])
            axis.text(j, i, f"{row['valid_runs']}/{row['planned_runs']}", ha="center", va="center", fontsize=8, color="white" if array[i, j] > 65 else "black")
    axis.set_xlabel("Problem class and number of operations")
    axis.set_title("Valid completions / all planned runs · failures retained")
    fig.colorbar(plot, ax=axis, label="Valid completions (%)", shrink=.85)
    save(fig, "completion_heatmap")

    fig, axes = plt.subplots(1, 3, figsize=(10.5, 4.5))
    fig.subplots_adjust(left=.06, right=.99, bottom=.27, top=.82, wspace=.35)
    rows = read("quality_time")
    handles = []
    for axis, kind in zip(axes, classes):
        for method in methods:
            row = next(r for r in rows if r["problem_class"] == kind and r["method"] == method)
            point = axis.scatter(float(row["mean_seconds"]), float(row["mean_hv_deficit_pct"]), color=colours[method], marker="^" if method == "XG" else "o", s=40, edgecolor="#333333", linewidth=.4, label=method)
            if kind == "JSP": handles.append(point)
        axis.set(xscale="log", title=kind, xlabel="Mean internal time (s; log scale)", ylabel="Mean HV deficit (%)")
        axis.grid(color="#dddddd", linewidth=.5)
    fig.legend(handles, methods, loc="lower center", bbox_to_anchor=(.5, .01), ncol=6, frameon=False, fontsize=8)
    fig.suptitle("Paired-instance means · concurrent host · XG uses one construction", fontsize=11)
    save(fig, "quality_time")

    rows = read("ecdf")
    fig, axes = plt.subplots(1, 3, figsize=(10.5, 4.5))
    fig.subplots_adjust(left=.06, right=.99, bottom=.27, top=.82, wspace=.35)
    handles = []
    for axis, kind in zip(axes, classes):
        for j, method in enumerate(methods):
            group = [r for r in rows if r["problem_class"] == kind and r["method"] == method and r["metric"] == "hv_deficit_pct"]
            x, y = [float(r["threshold"]) for r in group], [float(r["fraction_leq"]) for r in group]
            line, = axis.step([0] + x, [0] + y, where="post", color=colours[method], linestyle="--" if j >= 8 else "-", label=method)
            if kind == "JSP": handles.append(line)
        axis.set(title=kind, xlabel="HV deficit threshold (%)", ylabel="Fraction of paired instances", ylim=(0, 1.02))
        axis.grid(color="#dddddd", linewidth=.5)
    legend_lines = [Line2D([0], [0], color=colours[method], linestyle="--" if j >= 8 else "-") for j, method in enumerate(methods)]
    fig.legend(legend_lines, methods, loc="lower center", bbox_to_anchor=(.5, .01), ncol=6, frameon=False, fontsize=8)
    fig.suptitle("Empirical cumulative distributions · higher curves reach a threshold more often", fontsize=11)
    save(fig, "quality_ecdf")

    rows = read("convergence_summary")
    fig, axes = plt.subplots(1, 3, figsize=(10.5, 4.5))
    fig.subplots_adjust(left=.06, right=.99, bottom=.27, top=.82, wspace=.35)
    handles = []
    for axis, kind in zip(axes, classes):
        for j, method in enumerate(methods):
            # Keep a fixed population in the displayed curve; early points with
            # incomplete common support remain available in the data, not the plot.
            group = [r for r in rows if r["axis"] == "evaluations" and r["problem_class"] == kind and r["method"] == method and r["common_available_instances"] == r["terminal_paired_instances"]]
            line, = axis.plot([float(r["checkpoint"]) for r in group], [float(r["mean_hv_deficit_pct"]) for r in group], color=colours[method], linestyle="--" if j >= 8 else "-", label=method)
            if kind == "JSP": handles.append(line)
        axis.set(title=kind, xscale="log", xlabel="Evaluation allowance (log scale)", ylabel="Mean HV deficit (%)")
        axis.grid(color="#dddddd", linewidth=.5)
    fig.legend(handles, methods, loc="lower center", bbox_to_anchor=(.5, .01), ncol=6, frameon=False, fontsize=8)
    fig.suptitle("Observed convergence · fixed paired population · XG remains at its single construction", fontsize=11)
    save(fig, "convergence_evaluations")
    (directory / "plot_manifest.json").write_text(json.dumps({"plots": saved, "formats": ["pdf", "svg", "png"], "matplotlib": matplotlib.__version__, "solver_runs": 0}, indent=2) + "\n")
    print(json.dumps({"rendered": saved}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    render(parser.parse_args().directory.resolve())
