# Public benchmark inputs

These files are versioned inputs, not a disposable download cache.

| Path | Contents |
|---|---|
| `scaling-instances.json` | All 88 selected instances with operation durations and eligible machines: 21 JSP, 39 FJSP, 28 PFSP |
| `scaling-selection.json` | All 280 candidates, deterministic selection ranks, inclusion decisions, source URLs and SHA-256 hashes |
| `raw/` | All 96 original source files needed to reconstruct the complete candidate frame and selection |

The two JSON files are byte-identical copies of the completed experiment's
`instances.json` and `selection.json` under
`../results/scaling-20260924-current-parallel/`. Original source files are
byte-identical to `snapshot/benchmark/data/raw/` inside `../algorithm-9bc9dbf.zip`.
`python -B reproduce.py --check`, from the benchmark directory, verifies these
copies and their source hashes. The publication inventory includes every file.

Source URLs and hashes are retained for every candidate, including candidates
that were not selected. Sources are OR-Library, Taillard's author-hosted datasets,
and SchedulingLab's Brandimarte and Behnke-Geiger distributions. Dataset and
method citations remain necessary; the APEX license does not relicense this data.

To reconstruct selection, work in a separate copy of the benchmark directory
and run the command below there. It reads bundled `data/raw` files and rewrites
only the two `scaling-*.json` files; it does not run optimisation. A fresh
download is unnecessary. The frozen run keeps the original Windows path
separators in source metadata; regeneration on another OS can change those
separators without changing the selected jobs or inclusion decisions.

```sh
python -B -c "from scaling_data import prepare_expanded; prepare_expanded()"
```

For a fresh solver experiment, follow `../README.md` and use `reproduce.py`.
Do not use the historical pilot command `instances.py` to select this campaign.
