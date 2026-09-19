# Aniimax

A command-line tool, Rust library, and **web application** for optimizing production paths in Aniimo Homeland. Calculate the fastest way to produce your target amount of Homeland currency, and see what every facility you own should be doing at once.

Updated for the full release, with a joint LP-based facility-allocation engine for the web app (the CLI uses a simpler greedy approach; see [How the Optimization Works](#how-the-optimization-works) for the difference). Game data is being re-verified against the release facility by facility; facilities whose data hasn't been confirmed yet are left out until it is, so the calculator never recommends numbers from an older version of the game.

> **Note:** Game data for the full release is still being filled in, so some facilities and items are missing.

## Try It Online

**[Launch Aniimax Web App](https://ae-bii.github.io/aniimax/)** - No installation required!

## Features

**Web app**
- **Simple or Advanced Setup**: Simple mode only asks for your RV level and assumes everything that level allows is built and upgraded; advanced mode sets every facility's count and level (and can start from the simple-mode setup)
- **Live Production Plan**: Set your facilities to get the best achievable rate and what every facility should produce; no target amount needed
- **Goal Timing**: Add a target amount afterward to see how long it'll take; updates instantly as you type, no re-solving
- **Proven Best Plans**: The web app solves the whole problem exactly (every recipe, whole plots and machines, and environment building layouts together) with the [HiGHS](https://highs.dev) solver, and proves each plan is the best possible for your facilities
- **Joint Facility Allocation**: Solves for every item and every facility at once, so shared resources (e.g. two recipes both wanting the same Farmland soybean supply) are split correctly instead of double-counted
- **Whole-Unit Realism**: Growers are rounded to whole plots and processors are dedicated to one recipe each, matching how the game actually works; only the Woodworking Bench and Chimney Kiln take turns between tiers, since each tier is made from the one below
- **Level-Up Strategy**: Plans the soonest next RV level-up (coins plus Wood Blocks and Mineral Sand, or from RV 7 the Woodworking Bench and Chimney Kiln items it costs), counting what you already have, then earns as many coins as that pace allows; RV 2 to 20
- **Byproduct Priority**: With the Most coins strategy, optionally guarantee the maximum Wood Blocks/Mineral Sand rate first, even at some cost to Coins
- **Recipe Reference Page**: Every recipe in the game data, browsable by facility, independent of what you own
- **Aniimo Recommendations**: Every plan is solved twice, for the Best Aniimo (level 3 with the facility's personality bonus everywhere) and the Minimum (the lowest ability level each recipe accepts), and lists the Aniimo team it needs: each ability, level and personality, and how many it takes to keep up with the work (Farmland and Woodland jobs included), checked against how many Aniimo your RV level allows; times follow the game's Efficiency: 100% (one workload per second) at the level a recipe needs; at a processor, 300% one level above, then +100% per level; at gathering facilities (Well, Mine, Sandcastle and the like), +50% per level above a level-1 recipe and +40% per level above a harder one; the personality bonus adds 20%
- **Item Upgrade Modules**: Support for module-unlocked items (Ecological, Kitchen, Resource Detector, Crafting)

**CLI / library**
- **Time or Energy Optimization**: Fastest path, or best profit per energy unit
- **Energy Self-Sufficient Mode**: Produce items to consume for energy instead of buying
- **Cross-Facility Parallel Mode**: Run independent, non-conflicting production chains simultaneously
- **Optimal Facility Allocation**: Binary-search-based splitting when one recipe needs multiple materials from the same facility (e.g. rose + lavender for bouquet)
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

# Maximize Wood Blocks instead of coins
cargo run --release -- --target 500 --currency wood_blocks
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
  -c, --currency <CURRENCY>          What to optimize for: coins, or a byproduct
                                     (wood_blocks or mineral_sand) [default: coins]
  -e, --energy-cost <ENERGY_COST>    Energy cost per minute [default: 0.0]
      --energy-self-sufficient       Produce items to consume for energy
      --parallel                     Run different facility types simultaneously

  Facility counts:
      --farmland <N>                 Number of Farmland plots [default: 1]
      --woodland <N>                 Number of Woodland plots [default: 1]
      --mine <N>                     Number of Mine slots [default: 1]
      --well <N>                     Number of Wells [default: 0]
      --tidewhisper-sandcastle <N>   Number of Tidewhisper Sandcastles [default: 0]
      --carousel-mill <N>            Number of Carousel Mill machines [default: 1]
      --claw-game-cooker <N>         Number of Claw Game Cookers [default: 1]
      --jukebox-dryer <N>            Number of Jukebox Dryer machines [default: 1]
      --crafting-table <N>           Number of Crafting Table slots [default: 1]
      --simmering-pot <N>            Number of Simmering Pots [default: 0]

  Facility levels:
      --farmland-level <N>           Farmland facility level [default: 1]
      --woodland-level <N>           Woodland facility level [default: 1]
      --mine-level <N>               Mine facility level [default: 1]
      --well-level <N>               Well facility level [default: 1]
      --tidewhisper-sandcastle-level <N>
                                     Tidewhisper Sandcastle facility level [default: 1]
      --carousel-mill-level <N>      Carousel Mill facility level [default: 1]
      --claw-game-cooker-level <N>   Claw Game Cooker facility level [default: 1]
      --jukebox-dryer-level <N>      Jukebox Dryer facility level [default: 1]
      --crafting-table-level <N>     Crafting Table facility level [default: 1]
      --simmering-pot-level <N>      Simmering Pot facility level [default: 1]

  Aniimo:
      --aniimo-level <N>             Ability level (1-3) of the Aniimo working the Mine, Well,
                                     Tidewhisper Sandcastle and processors [default: 1]
      --personality-bonus            The working Aniimo has each facility's personality
                                     bonus (+20% speed)

  Item upgrade modules:
      --ecological-module <N>        Ecological Module level (unlocks quick crops) [default: 0]
      --kitchen-module <N>           Kitchen Module level (unlocks premium dishes) [default: 0]
      --resource-detector <N>        Resource Detector level (unlocks quick gathered items) [default: 0]
      --crafting-module <N>          Crafting Module level (unlocks premium crafts) [default: 0]

  -h, --help                         Print help
  -V, --version                      Print version
```

> **CLI coverage:** the CLI exposes the 10 facilities listed above; any facility without a flag counts as not owned. The CLI also doesn't model environment coverage, so it can recommend a crop that needs a Heat Furnace, Cooling Unit or Sunlamp you don't own. For full coverage, use the [web app](https://ae-bii.github.io/aniimax/) instead.

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
  Mine:               1 x Lv.1
  Well:               0 x Lv.1
  Tidewhisper:        0 x Lv.1
  Carousel Mill:      2 x Lv.2
  Claw Game Cooker:   1 x Lv.1
  Jukebox Dryer:      1 x Lv.1
  Crafting Table:     1 x Lv.1
  Simmering Pot:      0 x Lv.1

Item Modules:
  Ecological Module:  Lv.0
  Kitchen Module:     Lv.0
  Resource Detector:  Lv.0
  Crafting Module:    Lv.0

Aniimo:             Lv.1 suitability

Loaded 194 production items.

+================================================================+
|           ANIIMO PRODUCTION OPTIMIZATION RESULTS              |
+================================================================+

[BEST PRODUCTION PATH]
----------------------------------------------------------------
  Step 1: Produce 396 x rice at Farmland (x4)
  Step 2: Produce 22 x milled_rice at Carousel Mill (x2)

[SUMMARY]
----------------------------------------------------------------
  Total Profit:     5016 coins
  Total Time:       4h 20m 27s
    - Startup:      40m 27s (first batch)
    - Steady-state: 3h 40m 0s
  Items Produced:   22

[ALL OPTIONS RANKED] (by time efficiency)
----------------------------------------------------------------
Item                   Profit/sec Profit/energy    Time/unit
----------------------------------------------------------------
milled_rice                0.3800          N/A      40m 27s
tofu                       0.3633          N/A      40m 27s
...
```

## How the Optimization Works

The web app and the CLI/library use different approaches to the same underlying problem.

### Web App: Exact Planner

The web app builds the whole problem as one mixed-integer program (`src/exact.rs`) and solves it with [HiGHS](https://highs.dev), compiled to WebAssembly and run in the page's worker (`web/vendor/highs`, MIT license):

- **Recipes and units.** Every available recipe gets a rate (batches/sec) and a whole number of units set to it: plots for a crop, machines for a processed item. A unit makes one thing and is left running, so its rate is at most `units / time per batch`.
- **Item balances.** Everything made covers what other recipes use plus what's sold. A quick variant makes the same item as the regular one, and any leftover sells.
- **Facilities.** Each facility's units add up to at most what's owned, counting only units at a high enough level for each recipe.
- **Growing environments.** Each Heat Furnace, Cooling Unit and Sunlamp runs one mode and one coverage mix, from every undominated way one building can cover Farmland, Woodland and the rest (worked out once by exact packing); a crop needing an environment needs its plots covered.
- **Byproducts.** Wood Blocks and Mineral Sand balance like any other item, so the Woodworking Bench and Chimney Kiln can use them. Their recipes (level-up materials) take turns on the same unit instead of getting whole units each.
- **Objective.** Coins/sec from everything sold, minus seed costs. With byproducts prioritized, the most of each byproduct is found first and the plan must keep making that much.
- **Level-up.** The most level-ups per day ("pace") the plan could keep up: coins earned plus `pace x stock` must cover `pace x cost` for coins and every item, which stays linear. A second solve then finds the most coins at that pace, and a third puts spare Bench and Kiln time into more of what the level-up costs, so e.g. plentiful Mineral Sand ends up as ore rather than sitting raw.

HiGHS either proves its plan optimal, which the page reports, or stops at a time limit and reports how far from optimal it could be. Before a plan is shown, the whole-unit counts are re-solved with `microlp` and every limit is re-checked independently (`check_plan`); if anything fails, the page falls back to the heuristic planner below.

### Web App Fallback: Joint Facility Allocation

The heuristic planner (`find_plan`, backed by `find_production_plan`) solves a harder version of the problem than "what's the single best item": it solves for what *every* owned facility should be doing at once, including facilities that multiple recipes want to share.

**1. Profit per item.** For every item, net profit per batch, plus its utilization (batches/sec needed) at every facility touched anywhere in its ingredient chain, not just its own facility, but every intermediate processing step too.

```math
\text{profit}_{\text{batch}} = (\text{yield} \times \text{sell\_price}) - \text{raw\_cost}
```

**2. One linear program across everything.** Picking each item's rate independently would double-count facilities that two recipes both want (e.g. tofu and roasted soybeans both drawing from the same Farmland soybean supply). So every candidate item and every owned facility go into a single linear program instead, solved exactly with the [`microlp`](https://crates.io/crates/microlp) crate:

```math
\max \sum_i \text{profit}_{\text{batch},i} \cdot x_i \quad \text{s.t.} \quad \sum_i \text{utilization}_{i,f} \cdot x_i \leq \text{capacity}_f \ \ \forall f
```

**3. Rounding to whole units.** The LP's solution is continuous (e.g. "62% of Farmland grows soybean"), which isn't achievable in-game; plots and machines can't be fractionally split. The result is rounded differently depending on facility type:

- **Growers** (Farmland, Woodland, Mine, ...): each plot commits to one crop for a full cycle, so fractional shares are converted to whole counts via the largest-remainder method (the same apportionment technique used to allocate parliament seats).
- **Processors** (Carousel Mill, Claw Game Cooker, ...): a machine can't time-share between two recipes either; a player sets it to run one recipe continuously. When more recipes want a processor than it has units, the most profitable candidates each get one dedicated unit and the rest are excluded, then the LP re-solves so their freed-up supply finds a real next-best use instead of sitting idle.
- **Filling the whole units**: once the counts are settled, the LP solves one last time with each item capped at its whole units. A plot or Well rounded up produces its full output, not the fraction the continuous solve needed, and every chain using the same item shares the same units, so the extra goes to whichever recipe can use it (a spare processor unit can take a new recipe) or sells directly. Units the rounding left idle grow the facility's most valuable crop to sell.

**4. Time to reach a goal.** Once the plan is settled, each item contributes nothing until its own lead time has passed, then its steady rate. The time to reach a target amount is found with a binary search rather than solved for directly, since accumulated amount is monotonic in time:

```math
\text{amount}(t) = \sum_i \text{rate}_i \cdot \max(0,\ t - \text{lead}_i)
```

See the "math" button in the web app's header for this same explanation in context, or [`optimizer.rs`](src/optimizer.rs) (`find_production_plan`, `solve_facility_allocation`, `time_to_reach_goal`) for the implementation.

### CLI / Library: Greedy Path Selection

The CLI and library functions (`find_best_production_path`, `find_parallel_production_path`) use a greedy algorithm instead of the web app's joint solve, ranking items independently rather than solving for shared facilities at once. Here's how it works. The worked examples use illustrative numbers from an earlier version of the game data; the mechanics they demonstrate are unchanged.

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

**Quick Variants:**

When calculating raw material requirements, the optimizer automatically uses a quick variant (like `quick_wheat` in place of `wheat`) if you have the required module level. Quick variants sell for the same price per unit but yield more, making processed items more efficient. The substitution is by name: a recipe calling for `X` is supplied by `quick_X` whenever it's unlocked.

### 2. Item Filtering

Items are filtered based on your configuration:

- **Facility levels**: Only items unlocked at your facility level are considered
- **Module levels**: Upgraded items (like quick wheat) require the corresponding module at the right level
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
  Step 1: Produce 57240 x quick_wheat at Farmland (x20)
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
    → 50640 x quick_wheat at Farmland (x20) (raw material)
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

Using quick_wheat (yield 15, time 90s) with ecological_module:

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
        ("Mine", 1, 1),
        ("Carousel Mill", 2, 2),   // 2 carousel mills at level 2
        ("Jukebox Dryer", 1, 1),
        ("Crafting Table", 1, 1),
    ]);

    // Define item upgrade module levels (0 = not unlocked)
    let modules = ModuleLevels {
        ecological_module: 1,    // Unlocks quick wheat
        kitchen_module: 0,
        resource_detector: 0,
        crafting_module: 1,      // Unlocks premium river-washed stones
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

- `farmland.csv` - Crops (wheat, potato, rice, ...); also sets seed cost and growing environment
- `woodland.csv` - Trees (willow wood, bamboo, cocoa, ...); also yields Wood Blocks
- `mine.csv` - Mining (rock, clay, quartz ore, gem, ...); also yields Mineral Sand
- `well.csv` - Water (well water, fresh water, spring waters)
- `tidewhisper_sandcastle.csv` - Sea salt and pearl
- `dewy_house.csv`, `nimbus_bed.csv`, `starfall_hammock.csv`, `floral_windmill.csv` - Aniimo materials (aromathyst, wool, petals, star, scales)
- `carousel_mill.csv` - Grain and flour processing
- `crafting_table.csv` - Crafting recipes
- `claw_game_cooker.csv` - Baked goods, candy and desserts
- `jukebox_dryer.csv` - Food drying
- `simmering_pot.csv` - Porridge, jams, syrups and sugars
- `phonolfactory_table.csv` - Incense, soap and perfume
- `bouncy_brew_keg.csv` - Teas, juices and drinks
- `blazing_stove.csv` - Cooked dishes and sweets
- `pickling_jar.csv` - Sauces, vinegars and candied fruit
- `joy_wheel_loom.csv` - Thread, yarn and fabric
- `woodworking_bench.csv`, `chimney_kiln.csv` - RV level-up materials from Wood Blocks and Mineral Sand (no sale value)

Farmland, Woodland, Mine, Well, Tidewhisper Sandcastle, Dewy House, Carousel Mill, Crafting Table, Claw Game Cooker, Jukebox Dryer, Simmering Pot, Phonolfactory Table, Bouncy Brew Keg, Woodworking Bench and Chimney Kiln are verified in game. The other six facilities' recipes haven't been checked in game yet: `data/unverified.csv` lists them, the recipe list marks each one, and a plan lists any it relies on.

### Adding New Items

To add new production items, edit the appropriate CSV file. The format varies by facility type - see existing entries for examples.

## Project Structure

```
src/
  lib.rs             - Library root with module exports
  main.rs            - CLI entry point
  models.rs          - Data structures
  data.rs            - CSV loading functions
  exact.rs           - Exact planner: the web app's mixed-integer model, and its checks
  coverage.rs        - Environment building coverage geometry and packing
  optimizer.rs       - Heuristic planner (the web app's fallback) and the CLI's greedy path
  display.rs         - CLI output formatting
  wasm.rs            - WebAssembly bindings
data/
  *.csv              - Production data files
web/
  index.html         - Optimizer page (facility plan, goal timing, math/help/facilities modals)
  facility-config.js - Shared facility list/categories
  app.js             - Page logic, including the facility recipe reference modal
  style.css          - Styling
  worker.js          - Web Worker running the wasm module and HiGHS
  vendor/highs/      - HiGHS solver compiled to WebAssembly (MIT license)
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

To add missing items or correct existing data:

1. Edit the appropriate CSV file in `data/`, following the existing format for that facility
2. If you add a new CSV, load it in both `src/data.rs` and `src/wasm.rs`
3. Run `cargo test`; the data checks flag misspelled ingredients, mismatched quick variants and out-of-range values
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
