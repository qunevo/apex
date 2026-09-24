// Render the validated JSON views as a static, filterable research workbook.
import fs from "node:fs/promises";
import path from "node:path";
import assert from "node:assert/strict";
import { Workbook, SpreadsheetFile } from "@oai/artifact-tool";

const directory = path.resolve(process.argv[2]);
const views = JSON.parse(await fs.readFile(path.join(directory, "views.json"), "utf8"));
const meta = JSON.parse(await fs.readFile(path.join(directory, "export_manifest.json"), "utf8"));
const output = path.join(directory, "outputs", "scaling-20260924");
const previews = path.join(output, "previews");
await fs.mkdir(previews, { recursive: true });
const wb = Workbook.create();
const ink = "#173747", muted = "#526675", accent = "#166C77";
const metrics = ["mean_hv_deficit_pct", "mean_makespan_gap_pct", "mean_flowtime_gap_pct", "mean_seconds"];
const counts = ["valid_runs", "planned_runs", "timeouts", "success_pct", "planned_instances", "errors"];
const labels = {
  problem_class: "Class", method: "Method", instance: "Instance", operation_band: "Operations band",
  family: "Dataset family", cohort: "Cohort", paired_instances: "Paired instances",
  planned_instances: "All instances", mean_hv_deficit_pct: "HV deficit (%)",
  mean_makespan_gap_pct: "MS gap (%)", mean_flowtime_gap_pct: "FT gap (%)", mean_seconds: "Mean time (s)",
  valid_runs: "Valid runs", planned_runs: "All runs", timeouts: "Timeouts", success_pct: "Success (%)",
  errors: "Errors", classes_with_paired_data: "Paired classes", mean_class_success_pct: "Class-mean success (%)",
  seed_median_hv_deficit_pct: "Median HV deficit (%)", seed_median_makespan_gap_pct: "Median MS gap (%)",
  seed_median_flowtime_gap_pct: "Median FT gap (%)", seed_median_seconds: "Median time (s)",
  seed_min_hv_deficit_pct: "Min HV deficit (%)", seed_max_hv_deficit_pct: "Max HV deficit (%)",
  seed_min_seconds: "Min time (s)", seed_max_seconds: "Max time (s)",
  best_makespan: "Best makespan", best_job_flowtime: "Best job flowtime", hypervolume: "Hypervolume",
  seconds: "Time (s)", process_seconds: "Process time (s)", first_valid_seconds: "First valid (s)",
  hv_deficit_pct: "HV deficit (%)", makespan_gap_pct: "MS gap (%)", flowtime_gap_pct: "FT gap (%)",
  method_internal: "Original method ID", source_json: "Original raw JSON", paired_complete: "Fully paired",
};
const titleCase = key => labels[key] ?? key.split("_").map(word => word[0].toUpperCase() + word.slice(1)).join(" ");
function column(n) {
  let s = "";
  while (n > 0) { n--; s = String.fromCharCode(65 + n % 26) + s; n = Math.floor(n / 26); }
  return s;
}
const specs = [
  ["overall", "Overall", "Overall / equal instance weights", [], "69 fully paired instances; coverage includes all 88. Lower gaps are better."],
  ["by_problem_class", "By class", "Results by problem class", ["problem_class"], "Seed medians, then equal instance weights within JSP, FJSP and PFSP."],
  ["by_size", "By size", "Results by class and instance size", ["problem_class", "operation_band"], "Fixed bands: 1-150, 151-500, 501+ operations. Empty paired groups stay blank."],
  ["by_family", "By family", "Results by dataset family", ["problem_class", "family"], "Descriptive family breakdown; quality uses fully paired instances within each group."],
  ["by_cohort", "By cohort", "Legacy and added cases", ["problem_class", "cohort"], "Cohorts use the same current implementation and protocol; no historical run times are mixed in."],
  ["overall_class_balanced", "Class balanced", "Overall / equal problem-class weights", [], "JSP, FJSP and PFSP each receive one-third weight. Supplementary, post-result perspective."],
  ["coverage", "Coverage", "Run completion and timeouts", [], "All 3,168 runs; no exclusion based on solver success."],
  ["per_instance", "Per instance", "Instance-level results and seed spread", ["instance", "problem_class"], "Medians/min/max over available seeds; only fully paired instances enter aggregate quality."],
  ["raw_runs", "Raw runs", "Raw run metrics", [], ""],
];
const checks = [];
const renderSpecs = [];
for (const [key, name, title, dimensions, note] of specs) {
  const rows = views[key];
  const allKeys = Object.keys(rows[0]);
  let priority = [...dimensions, "method", "paired_instances", ...metrics, ...counts];
  if (key === "overall_class_balanced") priority = ["method", "classes_with_paired_data", ...metrics, "mean_class_success_pct", "weighting"];
  if (key === "coverage") priority = ["method", "all_valid", "all_planned", "all_timeouts", "all_success_pct", ...allKeys];
  if (key === "per_instance") priority = [...dimensions, "method", "paired_complete", "seed_median_hv_deficit_pct", "seed_median_makespan_gap_pct", "seed_median_flowtime_gap_pct", "seed_median_seconds", "valid_runs", "planned_runs", "timeouts"];
  if (key === "raw_runs") priority = ["instance", "problem_class", "method", "seed", "status", "best_makespan", "best_job_flowtime", "hypervolume", "seconds", "hv_deficit_pct", "makespan_gap_pct", "flowtime_gap_pct", ...allKeys];
  const columns = [...new Set([...priority, ...allKeys])].filter(k => allKeys.includes(k));
  const sheet = wb.worksheets.add(name);
  sheet.showGridLines = false;
  sheet.tabColor = ["raw_runs", "per_instance"].includes(key) ? "#738691" : accent;
  const headerRow = key === "raw_runs" ? 1 : 5;
  const lastRow = headerRow + rows.length;
  const lastCol = column(columns.length);
  const region = sheet.getRange(`A1:${lastCol}${lastRow}`);
  region.format.font = { name: "Arial", size: 10, color: ink };
  region.format.rowHeight = 21;
  region.format.columnWidth = 15;
  if (headerRow > 1) {
    sheet.getRange("A1").values = [[title]];
    sheet.getRange("A1").format.font = { name: "Arial", size: 15, bold: true, color: ink };
    sheet.getRange("A1").format.rowHeight = 30;
    sheet.getRange("A2").values = [[note]];
    sheet.getRange("A3").values = [["Static snapshot | run scaling-20260924-current-parallel | metric definitions and provenance: ReadMe"]];
    sheet.getRange("A2:A3").format.font = { name: "Arial", size: 10, color: muted };
  }
  const matrix = [columns.map(titleCase), ...rows.map(row => columns.map(k => row[k] ?? null))];
  const address = `A${headerRow}:${lastCol}${lastRow}`;
  sheet.getRange(address).values = matrix;
  const table = sheet.tables.add(address, true, `Results_${key}`);
  table.showFilterButton = true;
  const header = sheet.getRange(`A${headerRow}:${lastCol}${headerRow}`);
  header.format.fill = ink;
  header.format.font = { name: "Arial", size: 10, bold: true, color: "#FFFFFF" };
  header.format.wrapText = true;
  header.format.rowHeight = 38;
  header.format.verticalAlignment = "center";
  columns.forEach((field, i) => {
    const letter = column(i + 1);
    const body = sheet.getRange(`${letter}${headerRow + 1}:${letter}${lastRow}`);
    const numeric = rows.some(row => typeof row[field] === "number");
    const fractional = /pct|seconds|hypervolume|flexibility/.test(field);
    body.format.numberFormat = numeric ? fractional ? "0.000" : "0" : "General";
    body.format.horizontalAlignment = numeric ? "right" : "left";
    const width = field === "source_json" ? 72 : field === "weighting" ? 55 : ["family", "instance"].includes(field) ? 23 : field === "cohort" ? 20 : field === "method" ? 15 : 16;
    sheet.getRange(`${letter}1:${letter}${lastRow}`).format.columnWidth = width;
    if (field === "timeouts" || field.endsWith("_timeouts")) {
      body.conditionalFormats.add("cellIs", { operator: "greaterThan", formula: 0, format: { fill: "#FFF0D8", font: { color: "#80531C" } } });
    }
  });
  sheet.freezePanes.freezeRows(headerRow);
  sheet.freezePanes.freezeColumns(Math.max(1, dimensions.length));
  assert.deepEqual(sheet.getRange(address).values, matrix);
  checks.push({ sheet: name, view: key, table_range: address, data_rows: rows.length, columns, values_verified: true });
  renderSpecs.push({ sheetName: name, range: `A1:${column(Math.min(columns.length, key === "raw_runs" ? 9 : 9))}${Math.min(lastRow, 19)}`, file: key });
}

const notes = [
  ["Source run", meta.source_run],
  ["Population", "88 instances; 12 methods; 3 seeds (19, 42, 73); 3,168 individual runs."],
  ["Paired quality", "69 fully paired instances: JSP 19, FJSP 32, PFSP 18. Every method succeeded for all three seeds."],
  ["Aggregation", "Within an instance: median over seeds. Across fully paired instances: arithmetic mean, equal instance weights."],
  ["Class balancing", "Supplementary view: average the JSP, FJSP and PFSP means with equal weights. Defined after seeing results."],
  ["Missing values", "Blank = unavailable, never zero. Per-instance summaries can use fewer than three successful seeds; valid count is shown."],
  ["Coverage", "All planned runs, including 262 timeouts, remain in the coverage columns. Quality summaries alone do not establish overall superiority."],
  ["Quality reference", "Gaps/deficits are percentages relative to the best observed value in this experiment, not a known optimum. Lower is better."],
  ["Objective units", "Makespan (MS) and total job flowtime (FT) use source processing-time units. They are not asserted to be seconds."],
  ["Archive metrics", "Best MS and best FT can come from different schedules. HV uses (MS/S, FT/(n*S)), reference (1.1, 1.1)."],
  ["Runtime", "Internal wall seconds. Eight concurrent single-worker processes share the host. Process time also includes external overhead."],
  ["Budgets", "XG: one construction. Searches: up to 1,000 candidate evaluations. A common process watchdog is 240 seconds."],
  ["Seed spread", "Min/max over the available three seeds is descriptive, not a confidence interval. XG quality repeats are deterministic."],
  ["Naming", "XG = FastPlanner; XH = Trainer; XT = tree search; XE = genetic algorithm. Sequential combinations: XHT, XHE, XTE, XHTE."],
  ["Original IDs", "F=XG; T=XH; B=XT; G=XE; T-B=XHT; T-G=XHE; B-G=XTE; T-B-G=XHTE. Baselines retain their names."],
  ["Raw evidence", "Raw runs: one row per run. Original raw JSON paths are relative to views-v1; traces, schedules and archives remain unchanged."],
  ["Provenance", "views-v1/export_manifest.json stores source SHA-256 hashes, code hash, mapping and definitions. export_validation.json verifies totals."],
  ["Refresh", "Static research snapshot generated from views.json. Regenerate via benchmark/export_views.py and export_views_workbook.mjs."],
  ["Evidence boundary", "No solver was rerun. These views do not replace the historical pilot in the current manuscript."],
];
const readme = wb.worksheets.add("ReadMe");
readme.showGridLines = false;
readme.tabColor = "#738691";
readme.getRange(`A1:B${notes.length + 2}`).format.font = { name: "Arial", size: 10, color: ink };
readme.getRange("A1").values = [["Reading the benchmark views"]];
readme.getRange("A1").format.font = { name: "Arial", size: 15, bold: true, color: ink };
readme.getRange("A1").format.rowHeight = 30;
readme.getRange(`A3:B${notes.length + 2}`).values = notes;
readme.getRange(`A3:B${notes.length + 2}`).format.wrapText = true;
readme.getRange(`A3:B${notes.length + 2}`).format.verticalAlignment = "center";
readme.getRange(`A3:B${notes.length + 2}`).format.rowHeight = 43;
readme.getRange(`A1:A${notes.length + 2}`).format.columnWidth = 24;
readme.getRange(`B1:B${notes.length + 2}`).format.columnWidth = 110;
readme.getRange(`A3:A${notes.length + 2}`).format.font = { name: "Arial", size: 10, bold: true, color: ink };
readme.freezePanes.freezeRows(2);
renderSpecs.push({ sheetName: "ReadMe", range: `A1:B${notes.length + 2}`, file: "readme" });

wb.recalculate();
const inspection = await wb.inspect({ kind: "region", sheetId: "Overall", range: "A5:H17", maxChars: 6000, tableMaxRows: 13, tableMaxCols: 8 });
await fs.writeFile(path.join(output, "workbook_inspection.txt"), inspection.ndjson ?? JSON.stringify(inspection));
for (const spec of renderSpecs) {
  const preview = await wb.render({ sheetName: spec.sheetName, range: spec.range, scale: 1.25, format: "png" });
  await fs.writeFile(path.join(previews, `${spec.file}.png`), new Uint8Array(await preview.arrayBuffer()));
}
const file = await SpreadsheetFile.exportXlsx(wb);
await file.save(path.join(output, "benchmark_views.xlsx"));
await fs.writeFile(path.join(output, "workbook_validation.json"), JSON.stringify({ passed: true, static_snapshot: true, source_json: "../../views.json", sheets: checks, previews: renderSpecs }, null, 2) + "\n");
console.log(JSON.stringify({ workbook: path.join(output, "benchmark_views.xlsx"), sheets: specs.length + 1, raw_rows: views.raw_runs.length, all_values_verified: true }));
