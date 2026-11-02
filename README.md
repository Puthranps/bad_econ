# Spatial economy

A spatial industrial economy with 16 goods, supply chains, and research-driven production methods, for up to one million households and 10,000 generated firms. Burn/CubeCL runs the same Rust kernels on WGPU and CPU; Bevy provides an optional viewer. The Rust toolchain and dependencies are pinned.

## Live society sandbox

The Puthranps fork includes a live, spatial agent sandbox inspired by cellular automata. Open the native game with:

```powershell
cargo run --features viewer -- life
```

Each square holds at most one agent and a local food stock. Agents affect their eight immediate neighbors; the edges wrap. Cooperators invest in local production, exploiters take more, and conditional agents respond to their neighbors' previous behavior. Energy supports survival and reproduction. Offspring occupy adjacent empty cells, inherit a strategy with occasional mutation, and receive energy from their parent. Starvation and aging leave space for later generations. Agents stay in their cells; growth spreads through births.

Pause, step, paint agents or food, and change the rules while the simulation runs. Space toggles playback; N steps; R resets the current seed and rules; C starts the next seed; S changes speed; G switches the map; L opens the learning panel. Brushes 1–5 paint cooperators, exploiters, conditional agents, food or empty space; brackets change the radius. Hover over a cell to inspect its identity, parent, age and energy. The toolbar and rule controls also accept clicks.

H opens the whole-run history: population by strategy, births, deaths, hunger, peak population, cumulative totals and a recent event log. Painting is recorded as an intervention, separately from natural births and deaths. The curves retain up to 512 samples across the run; totals count every tick and the peak remains exact. E exports the history, initial configuration, current rules, recent 64 events and final grid to a uniquely named JSON file in `target/life`. Older events can leave the bounded log; the export describes its sampling and retention limits. Reset starts a new history, so export a run before resetting if you want to keep it.

Starting strategies are drawn with roughly equal probabilities: 33% cooperator, 33% exploiter, 34% conditional. Green, red and blue identify strategies; a blue conditional agent may choose either cooperation or exploitation. Empty terrain is neutral in strategy view. The food view uses brightness to show resource stocks. [Measured runs and validation](docs/experiments/life/README.md) compare scarcity, abundance and changes to cooperation and exploitation.

The map shows the current GPU state. Resetting the same seed repeats the same starting conditions, so you can compare interventions. There is no scheduled collapse or victory condition. This is a toy ecological economy with explicit rules, rather than a demographic forecast or Conway's original birth/survival rule.

Each tick regrows and harvests food, then resolves contributions and exploitation from one shared snapshot. A contribution produces twice its energy cost for adjacent living agents. Exploiters take a bounded amount from cooperating neighbors and pay an extra unit of metabolism when exploitation is enabled. Conditional agents cooperate when at least half their living neighbors cooperated on the previous tick. Integer remainders rotate among recipients. An agent dies when it cannot pay that tick's metabolic cost or reaches the configured maximum age. Age gates reproduction; agents of all ages can gather and interact. After consumption, an eligible parent may fund one neighboring birth; competing parents resolve claims deterministically. Each GPU tick checks food, energy and population accounts before publishing. Painting is an external intervention that adds or removes resources and agents.

The learning panel connects an experiment to [Federico, Balakrishna and Janssen's 2025 study of conditional cooperation](https://doi.org/10.1371/journal.pcsy.0000040). Their model finds that local interaction and conditional contributions can sustain cooperation and clustering. Try placing exploiters within a cooperative patch and changing the cooperation benefit. Predict what will happen before running it. Our resource stocks, births and deaths differ from the paper's fully occupied lattice and strategy imitation, so this is an exploration inspired by the research, not a replication.

For reproducible GPU runs without a window:

```powershell
cargo run -- life-run --width 96 --height 64 --density 35 --seed 42 --days 1000 --output target/life/run.json
cargo run --features viewer -- life-check
```

`life-run` records the starting configuration, initial and final grids, every tick's totals, GPU name and elapsed time. The `scenario` object can be saved separately and loaded with `--scenario PATH` in either mode. `life-check` exercises the native viewer and captures its rendered frame. The live domain, GPU kernels, and viewer are separate modules; the industrial ledger and the older recorded society experiment remain available below.

## Run

```sh
cargo run -- run --households 1000000 --days 30
cargo run --features viewer -- view --households 1000000
cargo run -- run --economy agrarian --households 10000 --days 30
cargo run -- run --scenario scenarios/institutions.json --days 12
cargo run -- analyze --analysis scenarios/analysis.json --output /tmp/accounts.json
```

GPU and the industrial economy are the defaults. `--economy agrarian` selects the original food economy; loaded scenarios retain their own economy setting. `--seed` controls generated populations and resource geography, `--output PATH` saves JSON, and `--checkpoint PATH` / `--restore PATH` save and resume audited state. See `cargo run -- --help` for all options.

On Windows, Smart App Control can block locally compiled build scripts with error 4551. The development profile leaves `icu_normalizer_data` and `icu_segmenter_data` unoptimized, which resolved the blocked ICU build scripts on the audited Windows setup. Other dependencies retain their optimization settings. This workaround leaves Windows security settings unchanged; future unsigned outputs can still be blocked. See [Microsoft's Smart App Control guidance](https://support.microsoft.com/en-us/windows/security/threat-malware-protection/smart-app-control-frequently-asked-questions).

State stores actor metadata, active recipes, per-good deliveries and consumption, bank and loan balances, and research records in field rows on the host and GPU. Checkpoints use format 5 with the `ResearchFixedV3` research layout and state-hash version 6; checkpoints from earlier formats must be regenerated. Scenario format remains version 1. Existing three-good scenario files still load, with additional inventories and research inputs initialized to zero, and omitted resource maps default to generated geography.

The viewer starts paused: Space toggles running, N advances one day, R resets, G changes the display, and C compares the next seed. Simulation runs on a worker; the viewer renders aggregate snapshots rather than household entities.

A status banner confirms completed controls and reports failures, with short sound cues and a subtle 240 ms color pulse confined to the banner. Automatic days stay quiet. M toggles sound; V reduces flashes to a static tint. Feedback preferences survive reset. The generated WAV cues are embedded in the executable, so launching from another directory needs no asset setup. The Linux viewer additionally needs ALSA development libraries for audio.

Presentation geometry and statistics refresh when the published display or selected chart changes. Worker publications wake the window immediately; between changes, it updates at most every 250 ms when focused or 500 ms when unfocused. Banner pulses temporarily use 60 Hz updates. The simulation worker and audit cadence are independent of rendering.

## Experimental society

The Puthranps fork also contains a separate GPU society experiment. Run it and open its offline map:

```powershell
cargo run -- society --agents 128 --seed 42 --days 120 --output target/society/run.json --html target/society/map.html
Invoke-Item target/society/map.html
```

The map replays the recorded run. Select a day and a person to inspect their food, cash, work, and interactions; switch colors between activity, life stage, and food security. The charts track population, cooperation, employment, and shortages. Map blocks represent local neighborhoods, rather than geographic distances.

Open **Learn through experiments** below the charts for three plain-language lessons and downloadable paired scenarios. Predict an outcome, rerun both versions for the same number of days, then inspect individual agents as well as totals. The lessons draw on [conditional cooperation and learning (2024 preprint)](https://arxiv.org/html/2409.09509v1), [reputation and institutions (PNAS, 2024)](https://doi.org/10.1073/pnas.2408802121), and [productivity and wage bargaining (2023)](https://doi.org/10.1007/s00191-022-00805-3). Each identifies which mechanisms the toy model omits; these experiments do not reproduce the papers.

Agents can contribute to shared production, free-ride, reciprocate their neighbors' behavior, or imitate a successful neighbor. Local trade and employer-worker negotiation provide additional interactions. Consumption draws down food reserves; sustained shortages can cause starvation. Births transfer resources from a parent, children must mature before working, and new adults can fill job vacancies. A newborn gets a new identity even when it occupies a previously used map slot.

Project strategy and trading honesty are separate traits. Trust-based buyers remember their last cheating seller; escrow guarantees delivery with payment. Workers can reject a posted wage or make one counteroffer, subject to the firm's budget. Their minimum accepted wage falls during consecutive shortages, an explicit modeling assumption. Parents fund births with food and a quarter of their cash; cash estates go to the local firm. Food production, consumption, and any abandonment have separate accounts.

Use `--scenario PATH` to load a society configuration. The `scenario` object in a JSON report is a complete starting configuration that can be saved, edited, and rerun. These configurations are separate from the industrial economy's scenario files. The original `run`, `view`, and industrial ledger are unchanged.

This is a bounded toy model: local membership, population capacity, food price, and job counts are fixed, while the living population changes. Birth and starvation rules are explicit assumptions, not demographic estimates. Escrow is automatic and has no operating cost. A shortage or population decline is observable; revolts, migration, old-age deaths, and a scripted collapse event are not included. Reports contain every slot on every recorded day, including children, deaths, and vacancies. GPU food, cash, and population outcomes are audited after each day. Configurations support up to 4096 slots and 365 days, with reports limited to 262144 slot-days; generated populations reserve twice as many slots as starting people.

## Model

Each day follows opening → observation → proposal → commitment → work/payment → provision/purchase → physical close → accounts/decisions → audit.

- Households supply labor and consume bread daily. Industrial households also buy linen and cloth every 20 days, coats every 40 days, and furniture and glass every 60 days, on staggered schedules. Firms produce discrete packets; each firm has a household owner.
- Labor and goods markets run four matching rounds, considering at most eight eligible nearby candidates per actor. Matching follows deterministic cell and identity order.
- Deliveries belong to the buyer and take at least one day. Pending goods cannot be consumed or used in production; new output reaches the next day's market.
- Resources use exact unsigned 64-bit ticks. Transactions check all debits and authorization before posting. Daily audits enforce cash and goods identities; faults restore the last audited state.
- Optional institutions include payroll taxes, wage bargaining, banks, support, research, equipment, natural stocks, and strikes. Populations and actor identities are fixed.

[Scenario examples](scenarios/) cover shortages, transport, concentrated markets, institutions, and custom populations. Export a starting configuration with:

```sh
cargo run -- scenario --households 1000000 --output /tmp/scenario.json
```

A loaded scenario supplies its own seed and population settings.

## Goods and production

Inventory, research-input, consumption, and market arrays follow this order: grain, flour, bread, iron, coal, steel, flax, linen, cloth, coats, wood, tools, furniture, cotton, dye, glass. `FirmSeed.good` uses the corresponding Rust enum name, such as `"Steel"` or `"Coats"`. A firm's `Technique.input_ticks` is consumed for **each** ingredient per packet; both ingredients must be available before labor can be committed.

Grain, iron, coal, flax, wood, cotton, and dye are **natural** goods. Their producers require the matching local resource as well as labor and any recipe inputs. All other goods are **manufactured** and can be produced anywhere with sufficient labor and inputs. Tools and fuel used by extraction upgrades do not change a good's category.

The optional `resources` setting defaults to `"Generated"`. Generation creates four seeded square patches per natural good, each with side length `max(1, map_size / 4)`, using random draws independent of population generation. Generated natural producers are placed within matching patches; household and manufacturer placement follows the existing population rules. The concentrated preset puts one patch of each natural good around the shared center.

An explicit resource map replaces all generated patches. For example:

```json
"resources": {
  "Explicit": {
    "regions": [
      { "good": "Grain", "lower": { "x": 0, "y": 0 }, "upper": { "x": 250, "y": 250 } },
      { "good": "Iron", "lower": { "x": 100, "y": 100 }, "upper": { "x": 500, "y": 500 } }
    ]
  }
}
```

Regions must name natural goods and be nonempty rectangles within the map. Lower bounds are inclusive, upper bounds exclusive; overlaps make all listed resources available. `{"Explicit":{"regions":[]}}` describes a map with no natural resources. Automatic population generation rejects a missing region for any natural good it needs to produce. Explicit populations keep their supplied positions: a natural producer at an unsuitable site remains valid and may sell existing inventory, but plans no production, hires no production labor, and creates no output. Its nominal capacity is retained. Resource capacity is cached separately in GPU actor metadata at initialization and restoration; research, equipment, and optional natural-stock constraints can further limit production. Geography itself does not deplete or renew, and firms do not relocate.

| Industry | Basic inputs | Research method | Required technology |
| --- | --- | --- | --- |
| Grain, flax, cotton, dye | None | MechanicalHarvesting; adds tools | Mechanization |
| Flour | Grain | SteamMilling; adds coal | SteamPower |
| Bread | Flour | MechanicalBaking | Mechanization |
| Iron, coal | None | SteamMining; adds tools and coal | SteamPower |
| Steel | Iron + coal | BessemerProcess | Steelmaking |
| Linen | Flax | PowerLooms | PowerLooms |
| Cloth | Cotton + dye | PowerLooms | PowerLooms |
| Coats | Cloth + linen | SewingMachines | SewingMachines |
| Wood | None | Sawmills; adds tools | Mechanization |
| Tools | Iron + wood | MachineTools; replaces iron with steel | Steelmaking |
| Furniture | Wood + tools | FurnitureWorkshops | Mechanization |
| Glass | Coal | Glassworks | SteamPower |

These initial upgrades double packet output (capped at 100), reduce labor to two-thirds (rounded down, at least one minute), and preserve the base input quantity, with a minimum of one tick for newly introduced inputs. An upgrade can therefore stall a firm until its new ingredients arrive. Existing shipments keep their original good when a firm changes methods. The viewer's firm inspection shows its current method, output, and labor requirement.

Industrial presets create firm-owned research labs. Mechanization unlocks SteamPower and PowerLooms; SteamPower unlocks Steelmaking; PowerLooms and Steelmaking together unlock SewingMachines. Their capability bits are 1, 2, 4, 8, and 16 respectively. Projects spend available household research time, use deterministic probabilistic discovery, and stop spending resources once their outcomes are discovered. Preset projects require labor; custom projects can also consume goods through `inputs`.

To enable an upgrade in a custom scenario, add its enum name to the owning research lab's `methods` array and supply projects with the required outcome bits. Discovery, mastery, permission, and adoption must all be present. `"MasterAndAdoptPermitted"` automatically masters and adopts permitted discoveries; `"Manual"` uses the explicitly supplied access masks. Methods are considered in array order and the last eligible method becomes active at the next opening. The optional `production_requirements` mask still gates the firm's entire production separately. The compact [industrial scenario](scenarios/industrial.json) includes all goods and research labs for steel and coats.

## Code

- [Domain](src/domain.rs): scenario validation, populations, state, checkpoints, and observations.
- [Simulation](src/simulation/): GPU/CPU kernels and phase orchestration. Markets group offers by counterparty and identity; fixed spatial membership is cached while eligibility refreshes each round.
- [Ledger](src/ledger.rs) and [arithmetic](src/arithmetic.rs): canonical transactions and exact arithmetic contracts.
- [Analytics](src/simulation/analytics.rs): exact interval accounts from explicitly supplied analytical inputs.
- [Viewer](src/viewer.rs): Bevy controls, inspection, and visualization.

## Lean correspondence

Compared against `../marx-lean-proof` at `1ad0e02`. The executable contracts correspond to the following proof modules; the Rust tests check finite arithmetic and execution, rather than extending the Lean proofs to the runtime.

| Lean contracts | Implementation and regression coverage |
| --- | --- |
| `Basic`, `Productivity`, `Spectral`, `Conversion`, `Theorems`; simulator `Analytics`, `Solver` | Certified goods and occupational equations, external and normalized weights, research plans, labeled comparators, residual bounds, and context caches in [analytics](src/simulation/analytics.rs), checked in [analytics tests](tests/analytics.rs). Positive-vector certificates establish productivity without computing floating-point eigenvalues. Empty systems have empty certificates and exact empty solutions. |
| `Model.valuation`, `ValuationRecord` | [Valuation records](src/simulation/analytics/valuation.rs) attach context versions, raw occupational hours, and supporting capabilities to selected activities. [Valuation tests](tests/valuation.rs) cover orientation, declared graphs, independent diagnostics, and request serialization on GPU. |
| `Capabilities`, `Support`, `BooleanBoundary`; simulator `Execution.Readiness` | [Capability closure](src/simulation/research/capabilities.rs), [boundary elimination](src/simulation/research/capabilities/boundary.rs), and separate discovery/mastery/permission/adoption gates; [research tests](tests/research.rs). |
| `Accounting`, `Blocks`, `Units` | Signed inventory reconciliation and horizons, research input accounts, block inverses, boundary demand, lifetime conventions, unit conversion and rebasing; [retained account tests](tests/retained.rs). |
| `Sensitivity`, `Truncation` | Signed derivatives, finite changes, coefficient responses, residual errors and weighted Neumann bounds in [sensitivity](src/simulation/analytics/sensitivity.rs); [sensitivity tests](tests/sensitivity.rs). |
| Simulator `Ledger`, `Packets`, `Execution` | Grouped gross debits, authorization, atomic posting, packets, fractional claims, keyed 128-bit research draws, dependency scheduling and audited checkpoints; [contracts](tests/contracts.rs), [execution](tests/execution.rs), and [GPU replay](tests/gpu.rs). Deserialized bundles and probabilities pass through their constructors. |
| Simulator `Policies`, `Finance` | Household health/time, quotes, forecasts, production starts, equipment, stocks, research hazards, payroll, bargaining, lending, dated funding, support and bank settlement; [household](tests/households.rs), [policy](tests/policies.rs), [payroll](tests/payroll.rs), [bargaining](tests/bargaining.rs), [finance](tests/finance.rs), and [institution](tests/institutions.rs) tests. |

The optional `valuation` object in [the analysis example](scenarios/analysis.json) selects zero-based goods indices. Each record reports hours per unit of output using the request's scale; historical research spending and scalar labor weights do not change those hours. Capability labels are 32-bit masks. `"InputSupport"` follows strictly positive goods inputs; `{"Declared": [{"prerequisite": 0, "dependent": 1}]}` uses the supplied dependency graph. Omitting capability inputs reports `MissingData`; an explicitly empty label set reports `Available(0)`. Boolean support remains available when the numerical account is inadmissible or unresolved.

The runtime uses bounded integers, directed rational intervals, and versioned nonlinear tables. It does not execute arbitrary real-valued inputs, unbounded catalogues, or every institutional scenario from the inherited PDFs. The fixed spatial population and 16-good catalogue define the concrete economy; the analytical APIs accept separate finite catalogues. Lean's conditional scheduling theorem, empirical calibration, and arbitrary scenario completeness are not runtime certification claims.

## Verify and benchmark

```sh
cargo test --features viewer
cargo clippy --all-targets --features viewer -- -D warnings
cargo run -- benchmark --sizes 1000000 --warmup 2 --days 60 --output /tmp/benchmark.json
```

Compute tests run exclusively on GPU and compare phase states across workgroups, the original transaction scheduler, and independent integer oracles. The test helper always selects the GPU. Benchmarks record daily timings, event counts, memory, readback volume, and state hashes. The final state download is outside the timed interval.

Simulation buffers may use half the GPU memory available when the device is initialized, queried through WGPU's Metal, Vulkan, or DirectX 12 backend. Metal uses its recommended working set minus current allocations; on unified-memory devices, this is also bounded by available system RAM because CPU and GPU allocations share memory. Vulkan and DirectX 12 use the driver budget minus current usage. Vulkan falls back to its largest device-local heap when budget reporting is unavailable. Backends without memory reporting and CPU runs retain a 4 GiB fallback. Scenario estimates are checked against the selected device before simulation allocation, and reserved memory is checked after initialization and every phase. Run and benchmark reports include the budget and whether it was reported or a fallback.
