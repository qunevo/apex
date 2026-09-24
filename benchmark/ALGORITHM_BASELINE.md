# Algorithm baseline: 24 September 2026

The current benchmark campaign is based on the APEX algorithm as of **24 September 2026**, recorded in Git commit **[`9bc9dbf`](https://github.com/qunevo/apex/commit/9bc9dbfe9f9d169b1ea1abf1894004384c9be7d7)** (full SHA: `9bc9dbfe9f9d169b1ea1abf1894004384c9be7d7`).

Development continues after this baseline to improve scheduling quality and performance and to support more modeling capabilities and production scenarios. Those later changes are outside this benchmark baseline. Results for this campaign describe the recorded algorithm version, not the capabilities or performance of a later checkout.

The commit identifies the application algorithm. Benchmark runners, configurations, input data and generated results were excluded from that source-only commit. The [source ZIP](algorithm-9bc9dbf.zip) contains the measured source and original harness; the run manifest and hashes verify its bytes. The reproduction helper extracts it only into a new directory outside the repository. No unpacked historical algorithm tree is needed in `src` or the results directory. Earlier pilot evidence retains its separate provenance outside this publication tree.

Evaluate subsequent algorithm versions in separately identified runs with their own commit and configuration. Keep the existing baseline results unchanged so that later improvements and expanded capabilities can be compared explicitly.
