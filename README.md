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
      --crafting-table-level <N>     Crafting Table facility level [default: 1]
      --dance-pad-polisher-level <N> Dance Pad Polisher facility level [default: 1]
      --aniipod-maker-level <N>      Aniipod Maker facility level [default: 1]

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

**Profit per second:**

$$\text{Profit/sec} = \frac{(\text{sell\_value} \times \text{yield}) - \text{raw\_material\_cost}}{\text{total\_production\_time}}$$

**Profit per energy** (for energy optimization mode):

$$\text{Profit/energy} = \frac{\text{profit}}{\text{energy\_consumed}}$$

**Effective profit per second** (accounts for parallel production):

$$\text{Effective Profit/sec} = \frac{\text{profit}}{\text{production\_time} / \text{facility\_count}}$$

For processed items (like wheatmeal from wheat), the total production time includes the time needed to grow the raw materials:

$$t_{\text{total}} = t_{\text{processing}} + \frac{t_{\text{raw}} \times \text{required\_amount}}{\text{raw\_yield}}$$

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

$$t_{\text{effective}} = \frac{t_{\text{actual}}}{n_{\text{facilities}}}$$

This significantly impacts which items are most efficient.

### Example

With 4 Farmlands at level 3, producing rice:

- Rice yields 10 units in 810 seconds, selling for 10 coins each

$$\text{Profit} = 10 \times 10 = 100 \text{ coins}$$

$$t_{\text{effective}} = \frac{810}{4} = 202.5 \text{ seconds}$$

$$\text{Profit/sec} = \frac{100}{202.5} \approx 0.49 \text{ coins/sec}$$

Note: All 4 batches complete together, so you actually earn $4 \times 100 = 400$ coins in 202.5 seconds.

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

## License

MIT License - see [LICENSE](LICENSE) for details.
