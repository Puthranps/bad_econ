# Society experiment evidence

Source: model commit `3966037e9cad21ff1de41bfb2bb928c8b9a15b1b`, map/lessons commit `18ada0145ae480036915bb13336da291f9e25bb9`.

These are deterministic demonstrations of a toy model, not empirical social predictions or reproductions of the linked research papers. All eight cases start with the same 128 people generated from seed 42, 256 capacity slots, and 120 model days. The scenario files record every initial agent and setting. Only the settings in `summary.json` differ between cases. One seed is insufficient to establish robustness.

Windows / NVIDIA GeForce RTX 3090 / Vulkan. All model execution used the GPU. `*.json.gz` contains every agent slot, firm, and summary for every recorded day. Decompress `example-map.html.gz` to open the offline interactive map, including paper links and downloadable lesson scenarios.

| Case | Peak / final people | Births / deaths | Frauds | Food-shortage agent-days | Mean wage |
| --- | ---: | ---: | ---: | ---: | ---: |
| baseline | 174 / 128 | 87 / 87 | 146 | 435 | 4.00 |
| reciprocity-25 | 204 / 128 | 114 / 114 | 246 | 570 | 4.00 |
| reciprocity-75 | 142 / 115 | 59 / 72 | 75 | 363 | 4.00 |
| escrow | 174 / 128 | 86 / 86 | 0 | 430 | 4.00 |
| posted-12 | 181 / 129 | 99 / 98 | 98 | 538 | 2.00 |
| posted-18 | 180 / 129 | 99 / 98 | 98 | 531 | 2.00 |
| negotiated-12 | 174 / 128 | 87 / 87 | 146 | 435 | 3.76 |
| negotiated-18 | 174 / 128 | 87 / 87 | 146 | 435 | 3.76 |

The baseline produced 87 births and 87 starvation deaths, with 17 newborn identities later employed. Every day preserved the initial total of 5760 cash units and reconciled food production, consumption, and abandonment. Population changes matched births minus deaths; no child was employed and no deceased identity returned to life. The comparison script independently checks these report-level identities.

The negotiated cases use posted offer 2, reservation 4, maximum wage 8. Their mean wage was 3.7603 at both work outputs; increasing output changed stored food rather than pay in these runs. This reflects the chosen rules. Escrow has zero fraud by construction and no modeled operating cost; its outcome is not evidence that real institutions are free or infallible. Food-shortage agent-days counts daily unmet needs, including an agent's final day; it is not the number of unique hungry people.

## Verification

- `cargo test --features viewer --test society --test gpu -j 4 -- --test-threads=1`: 10 GPU tests passed (6 new society tests and 4 existing economy/ledger tests).
- New cases cover workgroup replay, input bounds, starvation/estates, fraud/refusal/escrow, funded births/no child labor, distinct identities after dead-slot reuse, replacement hiring, and wage counteroffers.
- `cargo clippy --all-targets --features viewer -j 4 -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `check-report.mjs`: script/data checks over actual GPU output passed for all 120 days, map capacity, chart coordinates, contribution percentages, dead/future identity selection, and eight scenario downloads. This uses a DOM stub, not a browser.
- Final executable regenerated the baseline identically and embedded all eight lesson scenarios in the HTML.

Browser rendering, playback interaction, and actual native downloads remain unverified. Computer Use ended because browser URL-policy enforcement is not supported for the current Windows browser. No browser-policy workaround was attempted.

## Reproduce

Build with `cargo build --features viewer -j 4`. Copy this directory into `target/society-evidence` in the checkout, decompress `baseline.json.gz` to `baseline.json`, then run `python target/society-evidence/run-experiments.py`. That script runs all eight cases on GPU and writes `summary.json`. Run `node target/society-evidence/check-report.mjs` for the script/data checks.

For the map:

```powershell
cargo run --features viewer -- society --scenario target/society-evidence/baseline-scenario.json --days 120 --output target/society/run.json --html target/society/map.html
Invoke-Item target/society/map.html
```

The map's Learn section links the 2024 conditional-cooperation preprint, the 2024 PNAS paper on institutions and reputation, and the 2023 paper on productivity and wage bargaining (with an open 2021 author conference version). Lessons identify which mechanisms the simulator omits.
