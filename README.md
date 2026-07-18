# Aniimax

A command-line tool, Rust library, and **web application** for optimizing production paths in Aniimo Homeland. Calculate the fastest way to produce your target amount of Homeland currency, and see what every facility you own should be doing at once.

Supports the current beta with the full facility roster, Bud Tickets, and a joint LP-based facility-allocation engine for the web app (the CLI uses a simpler greedy approach; see [How the Optimization Works](#how-the-optimization-works) for the difference).

> **Note:** This project is a work in progress. Not all in-game items are included yet, and production times are assumed to match the values displayed in-game.

## Try It Online

**[Launch Aniimax Web App](https://ae-bii.github.io/aniimax/)** - No installation required!

## Features

**Web app**
- **Live Production Plan**: Set your facilities and currency to get the best achievable rate and what every facility should produce; no target amount needed
- **Goal Timing**: Add a target amount afterward to see how long it'll take; updates instantly as you type, no re-solving
- **Joint Facility Allocation**: Solves for every item and every facility at once, so shared resources (e.g. two recipes both wanting the same Farmland soybean supply) are split correctly instead of double-counted
- **Whole-Unit Realism**: Growers are rounded to whole plots, processors are dedicated to one recipe each, matching how the game actually works, never a fractional or time-shared facility
- **Multi-Currency Support**: Optimize for Coins or Bud Tickets
- **Recipe Reference Page**: Every recipe in the game data, browsable by facility, independent of what you own
- **Item Upgrade Modules**: Support for module-unlocked items (ecological, kitchen, mineral, crafting)

**CLI / library**
- **Time or Energy Optimization**: Fastest path, or best profit per energy unit
- **Energy Self-Sufficient Mode**: Produce items to consume for energy instead of buying
- **Cross-Facility Parallel Mode**: Run independent, non-conflicting production chains simultaneously
- **Optimal Facility Allocation**: Binary-search-based splitting when one recipe needs multiple materials from the same facility (e.g. lavender + rose for dried_flowers)
- **Startup Time Tracking**: Shows first-batch delay vs steady-state production time

## Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (1.70 or later)

### Building from Source

```bash
git clone https://github.com/ae-bii/aniimax.git
cd aniimax
cargo build --release
```

The binary will be available at `target/release/aniimax`.

## Usage

### Basic Usage

```bash
# Make 10000 coins as fast as possible
cargo run --release -- --target 10000 --currency coins

# Make 500 Bud Tickets
cargo run --release -- --target 500 --currency bud_tickets
```

### With Facility Counts and Levels

Specify how many of each facility you have and their levels for accurate production calculations:

```bash
cargo run --release -- --target 5000 --currency coins \
    --farmland 4 --farmland-level 3 \
    --woodland 2 --woodland-level 2 \
    --carousel-mill 2 --carousel-mill-level 2
```

### With Item Upgrade Modules

Enable upgraded items by specifying your module levels:

```bash
cargo run --release -- --target 5000 --currency coins \
    --farmland-level 3 \
    --ecological-module 1 \
    --crafting-module 1
```

### Energy Optimization

Pure profit-per-energy ranking exists at the library level (`find_best_production_path(&efficiencies, target, true, 0.0, &counts)`) but isn't currently wired up to a CLI flag; the CLI always ranks by time.

### With Energy Cost

Factor in energy costs when ranking by time (nudges the time-based ranking by the energy cost penalty, and prints per-item energy recommendations at the end):

```bash
cargo run --release -- --target 2000 --currency coins --energy-cost 10
```

### All Options

```
Options:
  -t, --target <TARGET>              Target amount of currency to produce
  -c, --currency <CURRENCY>          Currency type (coins or bud_tickets) [default: coins]
  -e, --energy-cost <ENERGY_COST>    Energy cost per minute [default: 0.0]
      --energy-self-sufficient       Produce items to consume for energy
      --parallel                     Run different facility types simultaneously

  Facility counts:
      --farmland <N>                 Number of Farmland plots [default: 1]
      --woodland <N>                 Number of Woodland plots [default: 1]
      --mineral-pile <N>             Number of Mineral Pile slots [default: 1]
      --carousel-mill <N>            Number of Carousel Mill machines [default: 1]
      --jukebox-dryer <N>            Number of Jukebox Dryer machines [default: 1]
      --crafting-table <N>           Number of Crafting Table slots [default: 1]
      --nimbus-bed <N>               Number of Nimbus Bed slots (produces Wool/Petals) [default: 0]

  Facility levels:
      --farmland-level <N>           Farmland facility level [default: 1]
      --woodland-level <N>           Woodland facility level [default: 1]
      --mineral-pile-level <N>       Mineral Pile facility level [default: 1]
      --carousel-mill-level <N>      Carousel Mill facility level [default: 1]
      --jukebox-dryer-level <N>      Jukebox Dryer facility level [default: 1]
      --crafting-table-level <N>     Crafting Table facility level [default: 1]
      --nimbus-bed-level <N>         Nimbus Bed facility level [default: 1]

  Item upgrade modules:
      --ecological-module <N>        Ecological Module level (unlocks high-speed crops) [default: 0]
      --kitchen-module <N>           Kitchen Module level (unlocks super wheatmeal) [default: 0]
      --mineral-detector <N>         Mineral Detector level (unlocks high-speed rock) [default: 0]
      --crafting-module <N>          Crafting Module level (unlocks advanced crafts) [default: 0]

  -h, --help                         Print help
  -V, --version                      Print version
```

> **CLI facility coverage:** the CLI currently only exposes the 7 facilities listed above. Any facility not listed here (Claw Game Cooker, Bouncy Brew Keg, Phonolfactory Table, Joy Wheel Loom, and the newer Aniimo-material facilities) defaults to 1 owned at level 1 when computing efficiencies. For full coverage of every current facility, use the [web app](https://ae-bii.github.io/aniimax/) instead.

## Example Output

```
Aniimax - Aniimo Production Optimizer
================================================================

Configuration:
  Target:          5000 coins
  Energy Cost:     0/min
  Mode:            Time Optimization

Facilities (count x level):
  Farmland:           4 x Lv.3
  Woodland:           1 x Lv.1
  Mineral Pile:       1 x Lv.1
  Carousel Mill:      2 x Lv.2
  Jukebox Dryer:      1 x Lv.1
  Crafting Table:     1 x Lv.1
  Nimbus Bed:         0 x Lv.1

Loaded 13 production items.

+================================================================+
|           ANIIMO PRODUCTION OPTIMIZATION RESULTS              |
+================================================================+

[BEST PRODUCTION PATH]
----------------------------------------------------------------
  Step 1: Produce 53 x rice_plant at Farmland (x4)

[SUMMARY]
----------------------------------------------------------------
  Total Profit:     5035 coins
  Total Time:       13m 30s
    - Startup:      14s (first batch)
    - Steady-state: 13m 16s
  Total Energy:     19557
  Items Produced:   530

[ALL OPTIONS RANKED] (by time efficiency)
----------------------------------------------------------------
Item                   Profit/sec Profit/energy    Time/unit
----------------------------------------------------------------
rice_plant                 7.0370       0.2575          14s
wheat                      6.6667       0.1236           2s
...
```

## How the Optimization Works

The web app and the CLI/library use two different approaches to the same underlying problem.

### Web App: Joint Facility Allocation

The web app (`find_plan`, backed by `find_production_plan`) solves a harder version of the problem than "what's the single best item": it solves for what *every* owned facility should be doing at once, including facilities that multiple recipes want to share.

**1. Profit per item.** For every item, net profit per batch, plus its utilization (batches/sec needed) at every facility touched anywhere in its ingredient chain, not just its own facility, but every intermediate processing step too.

```math
\text{profit}_{\text{batch}} = (\text{yield} \times \text{sell\_price}) - \text{raw\_cost}
```

**2. One linear program across everything.** Picking each item's rate independently would double-count facilities that two recipes both want (e.g. soy sauce and tofu both drawing from the same Farmland soybean supply). So every candidate item and every owned facility go into a single linear program instead, solved exactly with the [`microlp`](https://crates.io/crates/microlp) crate:

```math
\max \sum_i \text{profit}_{\text{batch},i} \cdot x_i \quad \text{s.t.} \quad \sum_i \text{utilization}_{i,f} \cdot x_i \leq \text{capacity}_f \ \ \forall f
```

**3. Rounding to whole units.** The LP's solution is continuous (e.g. "62% of Farmland grows soybean"), which isn't achievable in-game; plots and machines can't be fractionally split. The result is rounded differently depending on facility type:

- **Growers** (Farmland, Woodland, Mineral Pile, ...): each plot commits to one crop for a full cycle, so fractional shares are converted to whole counts via the largest-remainder method (the same apportionment technique used to allocate parliament seats).
- **Processors** (Carousel Mill, Claw Game Cooker, ...): a machine can't time-share between two recipes either; a player sets it to run one recipe continuously. When more recipes want a processor than it has units, the most profitable candidates each get one dedicated unit and the rest are excluded, then the LP re-solves so their freed-up supply finds a real next-best use instead of sitting idle.

**4. Time to reach a goal.** Once the plan is settled, each item contributes nothing until its own lead time has passed, then its steady rate. The time to reach a target amount is found with a binary search rather than solved for directly, since accumulated amount is monotonic in time:

```math
\text{amount}(t) = \sum_i \text{rate}_i \cdot \max(0,\ t - \text{lead}_i)
```

See the "math" button in the web app's header for this same explanation in context, or [`optimizer.rs`](src/optimizer.rs) (`find_production_plan`, `solve_facility_allocation`, `time_to_reach_goal`) for the implementation.

### CLI / Library: Greedy Path Selection

The CLI and library functions (`find_best_production_path`, `find_parallel_production_path`) use a greedy algorithm instead of the web app's joint solve, ranking items independently rather than solving for shared facilities at once. Here's how it works:

### 1. Efficiency Calculation

For each producible item, the optimizer calculates key metrics:

**Raw Material Profit per Second:**

For raw materials (wheat, chestnut, rock, etc.), profit per second considers parallel production:

```math
\text{Profit/sec} = \frac{(\text{sell\_value} \times \text{yield}) - \text{cost}}{\text{production\_time} / \text{facility\_count}}
```

**Processed Item Profit per Second (Steady-State Throughput):**

For processed items (wheatmeal, potato_chips, etc.), the optimizer calculates the **steady-state throughput** based on the production bottleneck. In continuous production, raw material gathering and processing can happen in parallel - the slower of the two determines overall throughput.

```math
\text{Gathering Rate} = \frac{\text{raw\_facility\_count} \times \text{raw\_yield}}{\text{raw\_production\_time} \times \text{required\_amount}}
```

```math
\text{Processing Rate} = \frac{\text{processing\_facility\_count}}{\text{processing\_time}}
```

```math
\text{Batches/sec} = \min(\text{Gathering Rate}, \text{Processing Rate})
```

```math
\text{Profit/sec} = \text{Batches/sec} \times \text{net\_profit\_per\_batch}
```

This means adding more farms speeds up processed item production (until processing becomes the bottleneck), and adding more processing facilities speeds up production (until raw material gathering becomes the bottleneck).

**Profit per energy** (for energy optimization mode):

```math
\text{Profit/energy} = \frac{\text{profit}}{\text{energy\_consumed}}
```

**High-Speed Variants:**

When calculating raw material requirements, the optimizer automatically uses high-speed variants (like `high_speed_wheat` instead of `wheat`) if you have the required module level. These variants produce more yield in the same time, making processed items more efficient.

### 2. Item Filtering

Items are filtered based on your configuration:

- **Facility levels**: Only items unlocked at your facility level are considered
- **Module levels**: Upgraded items (like high-speed wheat) require the corresponding module at the right level
- **Raw material availability**: Processed items are only available if their raw materials can be produced

### 3. Path Selection

**Time Optimization Mode** (default):

- Items are ranked by effective profit per second
- The algorithm selects the most time-efficient item and calculates how many batches are needed to reach your target
- Multiple facilities of the same type allow parallel production, reducing effective time

**Energy Optimization Mode**:

- Items are ranked by profit per energy unit
- Useful when energy is your bottleneck rather than time

**Energy Self-Sufficient Mode**:

- First identifies the most energy-efficient consumable item (like wheat)
- Calculates how much of that item to produce and consume for energy
- Then produces profit items using the generated energy

### 4. Parallel Production

When you have multiple facilities (e.g., 4 Farmlands), production time is divided:

```math
t_{\text{effective}} = \frac{t_{\text{actual}}}{n_{\text{facilities}}}
```

This significantly impacts which items are most efficient.

### 5. Cross-Facility Parallel Mode

When enabled with `--parallel`, the optimizer finds all production chains that can run simultaneously without sharing any facilities. This mode uses a greedy algorithm to maximize combined profit.

**How it works:**

1. Calculate efficiency for all producible items
2. Sort by profit per second (descending)
3. Greedily select non-conflicting items:
   - Track ALL facilities used in each production chain (including intermediate processing)
   - Skip items that would conflict with already-selected chains
4. Run all selected chains in parallel

**Multi-Level Chain Detection:**

For complex items like `caramel_nut_chips` that require intermediate processing:
- `caramel_nut_chips` needs `nuts` + `maple_syrup`
- `nuts` (processed at Jukebox Dryer) needs `walnut` + `chestnut`
- Full chain: **Woodland → Jukebox Dryer → Jukebox Dryer**

The optimizer tracks ALL facilities in the chain, so it correctly detects that `caramel_nut_chips` uses the Jukebox Dryer twice and won't run it in parallel with other Jukebox Dryer items.

```math
t_{\text{total}} = \max(t_{\text{chain\_1}}, t_{\text{chain\_2}}, ...) + t_{\text{startup}}
```

```math
\text{Profit}_{\text{total}} = \text{Profit}_{\text{chain\_1}} + \text{Profit}_{\text{chain\_2}} + ...
```

**Startup Time:**

The total time includes a startup delay (the time to produce the first batch before steady-state begins). This is the maximum first-batch time across all parallel chains.

**Example**: Producing 100,000 coins with 20 Farmlands, 5 Carousel Mills, and 6 Woodlands

Without parallel mode (super_wheatmeal only):
```
[BEST PRODUCTION PATH]
  Step 1: Produce 57240 x high_speed_wheat at Farmland (x20)
  Step 2: Produce 477 x super_wheatmeal at Carousel Mill (x5)

[SUMMARY]
  Total Time:       4h 46m 12s
    - Startup:      3m 0s (first batch)
    - Steady-state: 4h 43m 12s
  Total Profit:     100170 coins
```

With parallel mode (multiple independent chains):
```
[PARALLEL PRODUCTION CHAINS]
  All chains run simultaneously. Total time = longest chain.

  Chain 1: Farmland → Carousel Mill (88410 coins in 4h 30m 0s)
    → 50640 x high_speed_wheat at Farmland (x20) (raw material)
    → 422 x super_wheatmeal at Carousel Mill (x5)

  Chain 2: Woodland (12240 coins in 4h 30m 0s)
    → 34 x chestnut at Woodland (x6)

[SUMMARY]
  Total Time:       4h 33m 0s
    - Startup:      3m 0s (first batch)
    - Steady-state: 4h 30m 0s
  Total Profit:     100650 coins
```

The parallel mode improves profit by utilizing the idle Woodland facility!

### 6. Optimal Facility Allocation

When a recipe requires multiple different raw materials from the **same facility type**, Aniimax calculates the optimal way to split your facilities to minimize total production time.

**Example**: Producing `dried_flowers` (requires 3 lavender + 3 rose) with 20 Farmlands

| Material | Batches Needed | Production Time |
|----------|---------------|-----------------|
| lavender | 666           | 5400s (1.5h)    |
| rose     | 666           | 8100s (2.25h)   |

**Naive split (10 each):**
```math
t = \max\left(\lceil\frac{666}{10}\rceil \times 5400, \lceil\frac{666}{10}\rceil \times 8100\right) = \max(67 \times 5400, 67 \times 8100) = 542700s
```

**Optimal split (8 lavender, 12 rose):**
```math
t = \max\left(\lceil\frac{666}{8}\rceil \times 5400, \lceil\frac{666}{12}\rceil \times 8100\right) = \max(84 \times 5400, 56 \times 8100) = 453600s
```

The optimal allocation saves **~25 hours** by giving more facilities to the slower-producing material (rose).

**Algorithm:**

The algorithm uses **binary search on candidate completion times**:

1. **Generate candidate times**: For each material $i$ with $B_i$ batches and time $t_i$, the possible completion times are $\lceil B_i / k \rceil \cdot t_i$ for $k = 1, 2, \ldots$. Using the divisor counting trick, there are only $O(\sqrt{B_i})$ distinct values.

2. **Binary search**: For each candidate time $T$, check if it's achievable:
   - For material $i$: max rounds $= \lfloor T / t_i \rfloor$
   - Min facilities needed $= \lceil B_i / r_i \rceil$ where $r_i$ is max rounds
   - Feasible if total facilities needed $\leq F$

3. **Allocate**: Once the optimal time is found, assign minimum facilities to each material and greedily distribute remaining facilities.

The objective is to minimize:

```math
\min \max_i \left(\lceil\frac{B_i}{f_i}\rceil \times t_i\right) \quad \text{s.t.} \quad \sum_i f_i = F
```

**Complexity**: $O(M \cdot \sqrt{B} \cdot \log(M \cdot \sqrt{B}))$ where $M$ = materials, $B$ = max batches.

**When it applies:**
- Multiple materials from the **same** facility (lavender + rose from Farmland)
- Different production times between materials

**Does NOT apply:**
- Materials from different facilities (no allocation needed)
- Single material recipes (all facilities make the same thing)

### Example: Raw Materials

With 4 Farmlands at level 3, producing rice:

- Rice yields 10 units in 810 seconds, selling for 10 coins each (cost: 5 coins per batch)

```math
\text{Net Profit} = (10 \times 10) - 5 = 95 \text{ coins per batch}
```

```math
t_{\text{effective}} = \frac{810}{4} = 202.5 \text{ seconds}
```

```math
\text{Profit/sec} = \frac{95}{202.5} \approx 0.47 \text{ coins/sec}
```

### Example: Processed Items

With 4 Farmlands and 2 Carousel Mills, producing super_wheatmeal (requires 120 wheat, sells for 210 coins):

Using high_speed_wheat (yield 15, time 90s) with ecological_module:

```math
\text{Gathering Rate} = \frac{4 \times 15}{90 \times 120} = 0.00556 \text{ batches/sec}
```

```math
\text{Processing Rate} = \frac{2}{60} = 0.0333 \text{ batches/sec}
```

Bottleneck is gathering (0.00556 < 0.0333):

```math
\text{Profit/sec} = 0.00556 \times 210 = 1.17 \text{ coins/sec}
```

Adding more farms increases the gathering rate until it matches or exceeds the processing rate.

### Computational Complexity

The table below describes the CLI/library's greedy functions above, not the web app's linear program (LP solve time depends on the solver and isn't a simple closed form, but is fast in practice, well under a second for the current item count).

Let $n$ = number of production items, $m$ = maximum chain depth, $f$ = facilities per chain, $k$ = selected parallel chains, $F$ = facility count, $M$ = number of materials in a recipe.

| Operation | Complexity | Description |
|-----------|------------|-------------|
| Efficiency calculation | $O(n \cdot m^2)$ | Recursive chain traversal for each item |
| Parallel mode selection | $O(n \log n + n \cdot f)$ | Sort + greedy selection with conflict detection |
| Facility allocation | $O(M \cdot \sqrt{B} \cdot \log(M\sqrt{B}))$ | Binary search on candidate times |
| Startup time calculation | $O(k)$ | Max over $k$ selected chains |

With ~64 items, shallow chains ($m \leq 3$), and typically $M \leq 3$ materials, the algorithm runs in sub-millisecond time.

## Library Usage

This crate can also be used as a library:

```rust
use aniimax::{
    data::load_all_data,
    optimizer::{calculate_efficiencies, find_best_production_path},
    models::{FacilityCounts, ModuleLevels},
    display::display_results,
};
use std::path::Path;

fn main() {
    // Load production data
    let items = load_all_data(Path::new("data")).unwrap();

    // Define facility counts and levels as (name, count, level) triples. Any facility not
    // listed here defaults to count=1, level=1.
    let counts = FacilityCounts::from_pairs(&[
        ("Farmland", 4, 3),        // 4 farmlands at level 3
        ("Woodland", 2, 2),        // 2 woodlands at level 2
        ("Mineral Pile", 1, 1),
        ("Carousel Mill", 2, 2),   // 2 carousel mills at level 2
        ("Jukebox Dryer", 1, 1),
        ("Crafting Table", 1, 1),
    ]);

    // Define item upgrade module levels (0 = not unlocked)
    let modules = ModuleLevels {
        ecological_module: 1,    // Unlocks high-speed wheat
        kitchen_module: 0,
        mineral_detector: 0,
        crafting_module: 1,      // Unlocks advanced wood sculpture
    };

    // Calculate efficiencies (per-facility levels and modules are used automatically)
    let efficiencies = calculate_efficiencies(&items, "coins", &counts, &modules);

    // Find optimal path
    if let Some(path) = find_best_production_path(&efficiencies, 5000.0, false, 0.0, &counts) {
        display_results(&path, &efficiencies, false);
    }
}
```

## Documentation

Generate and view the documentation:

```bash
cargo doc --open
```

## Web Development

### Building the Web App

1. Install wasm-pack:

   ```bash
   cargo install wasm-pack
   ```

2. Build the WASM module:

   ```bash
   ./build-wasm.sh
   # or manually:
   wasm-pack build --target web --out-dir web/pkg
   ```

3. Test locally:
   ```bash
   cd web && python3 -m http.server 8080
   ```
   Open http://localhost:8080 in your browser.

### Deploying to GitHub Pages

Deployment (`.github/workflows/deploy.yml`) runs on pushing a version tag (`v*`) or via manual workflow dispatch, not on every push to main. Tag a release (`git tag v0.14.1 && git push --tags`) or trigger the workflow manually to deploy. You can also deploy by hand by copying the contents of the `web/` directory (including a freshly built `web/pkg/`) to your gh-pages branch.

## Data Format

Production data is stored in CSV files in the `data/` directory:

- `farmland.csv` - Crops (wheat, potatoes, etc.)
- `woodland.csv` - Trees (chestnut, willow)
- `mineral_pile.csv` - Mining (rock, quartz, gem)
- `nimbus_bed.csv` - Wool and Petals (requires a matching Aniimo Family)
- `grass_blossom_mat.csv`, `starfall_hammock.csv`, `tidewhisper_sandcastle.csv`, `dewy_house.csv` - newer Aniimo-material facilities (levels/values still being confirmed)
- `carousel_mill.csv` - Grain/tofu processing
- `jukebox_dryer.csv` - Food drying
- `crafting_table.csv` - Crafting recipes
- `phonolfactory_table.csv` - Incense, soap, perfume
- `bouncy_brew_keg.csv` - Drinks and sauces
- `claw_game_cooker.csv` - Candy and prepared food
- `joy_wheel_loom.csv` - Fabric and thread

### Adding New Items

To add new production items, edit the appropriate CSV file. The format varies by facility type - see existing entries for examples.

## Project Structure

```
src/
  lib.rs             - Library root with module exports
  main.rs            - CLI entry point
  models.rs          - Data structures
  data.rs            - CSV loading functions
  optimizer.rs       - Optimization algorithms (both the web app's LP solve and the CLI's greedy path)
  display.rs         - CLI output formatting
  wasm.rs            - WebAssembly bindings
data/
  *.csv              - Production data files
web/
  index.html         - Optimizer page (facility plan, goal timing, math/help/facilities modals)
  facility-config.js - Shared facility list/categories
  app.js             - Page logic, including the facility recipe reference modal
  style.css          - Styling
  pkg/               - Built WASM module (generated)
tests/
  *.rs               - Integration tests
```

## Contributing

Contributions are welcome! Here's how you can help:

### Reporting Issues

- Check existing issues before creating a new one
- Include steps to reproduce the problem
- Mention your environment (OS, Rust version, browser if applicable)

### Adding Game Data

The easiest way to contribute is by adding missing items or correcting existing data:

1. Edit the appropriate CSV file in `data/`
2. Follow the existing format for that facility type
3. Test locally with `cargo run -- --target 1000`
4. Submit a pull request

### Code Contributions

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/your-feature`
3. Make your changes
4. Run tests: `cargo test`
5. Build WASM to verify: `wasm-pack build --target web --out-dir web/pkg`
6. Commit with a descriptive message
7. Push and open a pull request

### Development Setup

```bash
# Clone your fork
git clone https://github.com/<your-username>/aniimax.git
cd aniimax

# Build and test
cargo build
cargo test

# Build WASM for web testing
wasm-pack build --target web --out-dir web/pkg

# Start local server for web app
cd web && python3 -m http.server 8080
```

## License

MIT License - see [LICENSE](LICENSE) for details.
