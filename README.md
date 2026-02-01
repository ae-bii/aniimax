# Aniimax

A command-line tool, Rust library, and **web application** for optimizing production paths in Aniimo Homeland. Calculate the fastest or most energy-efficient way to produce your target amount of Homeland currency.

> **Note:** This project is a work in progress. Not all in-game items are included yet, and production times are assumed to match the values displayed in-game.

## Try It Online

**[Launch Aniimax Web App](https://ae-bii.github.io/aniimax/)** - No installation required!

## Features

- **Time Optimization**: Find the fastest production path to reach your currency goal
- **Energy Optimization**: Maximize profit per energy unit when energy is limited
- **Energy Self-Sufficient Mode**: Produce items to consume for energy instead of buying
- **Parallel Production**: Account for multiple facilities running simultaneously
- **Cross-Facility Parallel Mode**: Run different facility types at the same time (e.g., Farmland→Carousel Mill + Woodland→Jukebox Dryer)
- **Multi-Level Production Chains**: Supports complex chains like caramel_nut_chips (Woodland → Jukebox Dryer → Jukebox Dryer)
- **Optimal Facility Allocation**: Calculates how to split facilities when producing multiple materials (e.g., lavender + rose for dried_flowers)
- **Startup Time Tracking**: Shows first-batch delay vs steady-state production time
- **Multi-Currency Support**: Optimize for either coins or coupons
- **Per-Facility Level Filtering**: Set different levels for each facility type
- **Item Upgrade Modules**: Support for module-unlocked items (ecological, kitchen, mineral, crafting)
- **Web Interface**: Use directly in your browser with WebAssembly

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

# Make 500 coupons
cargo run --release -- --target 500 --currency coupons
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

Optimize for energy efficiency instead of speed:

```bash
cargo run --release -- --target 5000 --currency coins --optimize-energy
```

### With Energy Cost

Factor in energy costs when optimizing for time:

```bash
cargo run --release -- --target 2000 --currency coins --energy-cost 10
```

### All Options

```
Options:
  -t, --target <TARGET>              Target amount of currency to produce
  -c, --currency <CURRENCY>          Currency type (coins or coupons) [default: coins]
  -e, --energy-cost <ENERGY_COST>    Energy cost per minute [default: 0.0]
      --optimize-energy              Optimize for energy efficiency instead of time
      --energy-self-sufficient       Produce items to consume for energy
      --parallel                     Run different facility types simultaneously

  Facility counts:
      --farmland <N>                 Number of Farmland plots [default: 1]
      --woodland <N>                 Number of Woodland plots [default: 1]
      --mineral-pile <N>             Number of Mineral Pile slots [default: 1]
      --carousel-mill <N>            Number of Carousel Mill machines [default: 1]
      --jukebox-dryer <N>            Number of Jukebox Dryer machines [default: 1]
      --crafting-table <N>           Number of Crafting Table slots [default: 1]
      --dance-pad-polisher <N>       Number of Dance Pad Polisher machines [default: 1]
      --aniipod-maker <N>            Number of Aniipod Maker machines [default: 1]

  Facility levels:
      --farmland-level <N>           Farmland facility level [default: 1]
      --woodland-level <N>           Woodland facility level [default: 1]
      --mineral-pile-level <N>       Mineral Pile facility level [default: 1]
      --carousel-mill-level <N>      Carousel Mill facility level [default: 1]
      --jukebox-dryer-level <N>      Jukebox Dryer facility level [default: 1]
      --crafting-table-level <N>     Crafting Table facility level [default: 1]
      --dance-pad-polisher-level <N> Dance Pad Polisher facility level [default: 1]
      --aniipod-maker-level <N>      Aniipod Maker facility level [default: 1]

  Item upgrade modules:
      --ecological-module <N>        Ecological Module level (unlocks high-speed crops) [default: 0]
      --kitchen-module <N>           Kitchen Module level (unlocks super wheatmeal) [default: 0]
      --mineral-detector <N>         Mineral Detector level (unlocks high-speed rock) [default: 0]
      --crafting-module <N>          Crafting Module level (unlocks advanced crafts) [default: 0]

  -h, --help                         Print help
  -V, --version                      Print version
```

## Example Output

```
Aniimax - Aniimo Production Optimizer
================================================================

Configuration:
  Target:          5000 coins
  Energy Cost:     0/min
  Optimize for:    Time

Facilities (count x level):
  Farmland:           4 x Lv.3
  Woodland:           1 x Lv.1
  Mineral Pile:       1 x Lv.1
  Carousel Mill:      2 x Lv.2
  Jukebox Dryer:      1 x Lv.1
  Crafting Table:     1 x Lv.1
  Dance Pad Polisher: 1 x Lv.1
  Aniipod Maker:      1 x Lv.1

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

Aniimax uses a greedy algorithm to find efficient production paths. Here's how it works:

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

**How it works:**

For 2 materials, the algorithm tries all possible splits and selects the one that minimizes:

```math
\min_{f_1 \in [1, F-1]} \max\left(\lceil\frac{B_1}{f_1}\rceil \times t_1, \lceil\frac{B_2}{F-f_1}\rceil \times t_2\right)
```

Where:
- $F$ = total facilities available
- $B_i$ = batches needed for material $i$
- $t_i$ = production time for material $i$
- $f_i$ = facilities allocated to material $i$

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

Let $n$ = number of production items, $m$ = maximum chain depth, $f$ = facilities per chain, $k$ = selected parallel chains, $F$ = facility count, $M$ = number of materials in a recipe.

| Operation | Complexity | Description |
|-----------|------------|-------------|
| Efficiency calculation | $O(n \cdot m^2)$ | Recursive chain traversal for each item |
| Parallel mode selection | $O(n \log n + n \cdot f)$ | Sort + greedy selection with conflict detection |
| Facility allocation | $O(\binom{F+M-1}{M-1})$ | Optimal allocation via recursive search |
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

    // Define facility counts and levels: (count, level)
    let counts = FacilityCounts {
        farmland: (4, 3),        // 4 farmlands at level 3
        woodland: (2, 2),        // 2 woodlands at level 2
        mineral_pile: (1, 1),
        carousel_mill: (2, 2),   // 2 carousel mills at level 2
        jukebox_dryer: (1, 1),
        crafting_table: (1, 1),
        dance_pad_polisher: (1, 1),
        aniipod_maker: (1, 1),
    };

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

The web app is automatically deployed to GitHub Pages on every push to the main branch. You can also manually deploy by copying the contents of the `web/` directory to your gh-pages branch.

## Data Format

Production data is stored in CSV files in the `data/` directory:

- `farmland.csv` - Crops (wheat, potatoes, etc.)
- `woodland.csv` - Trees (chestnut, willow)
- `mineral_pile.csv` - Mining (rock)
- `carousel_mill.csv` - Grain processing
- `jukebox_dryer.csv` - Food processing
- `crafting_table.csv` - Crafting recipes
- `dance_pad_polisher.csv` - Special items
- `aniipod_maker.csv` - Aniipod production

### Adding New Items

To add new production items, edit the appropriate CSV file. The format varies by facility type - see existing entries for examples.

## Project Structure

```
src/
  lib.rs          - Library root with module exports
  main.rs         - CLI entry point
  models.rs       - Data structures
  data.rs         - CSV loading functions
  optimizer.rs    - Optimization algorithms
  display.rs      - Output formatting
  wasm.rs         - WebAssembly bindings
data/
  *.csv           - Production data files
web/
  index.html      - Web app entry point
  style.css      - Styling
  app.js          - JavaScript application
  pkg/            - Built WASM module (generated)
tests/
  *.rs            - Integration tests
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
git clone https://github.com/ae-bii/aniimax.git
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
