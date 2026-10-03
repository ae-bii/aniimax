# Contributing to Aniimax

Thanks for helping improve Aniimax. Bug reports, verified game data, tests, and code changes are welcome.

## Before you start

- Check existing issues and pull requests before opening a duplicate.
- For a bug, include the steps to reproduce it, what you expected, what happened, and the relevant configuration. Include your OS, browser, and Rust version when they matter.
- For game data, explain how you verified the values in the current game release. Mark values that have not been confirmed rather than presenting them as verified.
- Keep each pull request focused on one change, and link an issue when one exists.

## Find the right files

| Area | Files |
| --- | --- |
| Rust library and CLI | `src/`, with integration tests in `tests/*.rs` |
| Recipes and production data | `data/*.csv`, loaded by `src/data.rs` and embedded for the web app by `src/wasm.rs` |
| Browser app | `web/index.html`, `web/app.js`, `web/worker.js`, `web/style.css` |
| Browser facility definitions | `web/facility-config.js` |
| Browser tests | `tests/*.mjs` |

`web/facility-config.js` is the frontend source of truth for facilities. Keep its names in sync with Rust and CSV data when changing a facility. Preserve deterministic planning, whole-unit grower allocation, and one recipe per processor when changing optimization behavior.

Do not edit or commit generated `target/` or `web/pkg/` files.

## Set up locally

Install [Rust](https://www.rust-lang.org/tools/install). For browser work, also install Node.js (CI uses Node 22), Python 3, and `wasm-pack` or let `build-wasm.sh` install it.

```bash
git clone https://github.com/<your-username>/aniimax.git
cd aniimax
cargo build --locked
cargo test --release --locked
```

The full Rust suite includes computationally expensive layout coverage tests. Run it in release mode, as CI does, so those tests complete in a reasonable time. Three longer data-generation and sweep tests are intentionally ignored by the default suite; their test comments explain when to run them manually.

For browser work, build the WASM module before testing the page:

```bash
./build-wasm.sh
cd web
python3 -m http.server 8080
```

Open `http://localhost:8080` and exercise the affected UI. Use the local server rather than opening `index.html` with a `file://` URL, because the app loads JavaScript modules and workers. Run `node --test` from the repository root for JavaScript tests.

## Validate your change

| Change | Checks |
| --- | --- |
| Rust logic | `cargo build --locked`, `cargo test --release --locked`, `cargo fmt -- --check`, and `cargo clippy --all-targets --all-features` when available |
| Recipes or other CSV data | `cargo test --release --locked` and `cargo run -- --target 1000` |
| Browser or WASM code | `./build-wasm.sh`, `node --test`, and a check of the affected flow in the local browser |
| Documentation only | Check the rendered Markdown, links, and commands you changed |

Add tests alongside the relevant code: Rust unit tests in `src/`, domain-level integration tests in `tests/`, and JavaScript tests in `tests/*.mjs`. Name new Rust tests `test_<behavior>_<condition>` where practical.

For a new CSV file, wire it into both Rust loaders (`src/data.rs` and `src/wasm.rs`). When changing game data, include verification details in the pull request and check that item names, ingredients, facility names, and quick variants still match.

## Name your branch

Follow [Conventional Branch](https://conventionalbranch.org/) using `<type>/<description>`. Choose a prefix based on the work:

| Prefix | Use for | Example |
| --- | --- | --- |
| `feature/` or `feat/` | New behavior | `feature/shareable-config-link` |
| `bugfix/` or `fix/` | Bug fixes | `bugfix/incorrect-coin-total` |
| `hotfix/` | Urgent fixes | `hotfix/restore-deployment` |
| `release/` | Release preparation | `release/v0.17.0` |
| `chore/` | Documentation, dependencies, and other maintenance | `chore/update-contributor-guide` |

Use lowercase letters and numbers, with single hyphens between words. Dots are for version numbers in release branches. Do not use spaces, underscores, uppercase letters, consecutive separators, or a separator at the start or end of the description. Keep the description short and specific; include an issue number when it helps trace the work, such as `feature/issue-123-add-recipes`. The trunk branch `main` has no prefix.

## Open a pull request

1. Create a branch using the naming rules above and commit only the files needed for the change. Use a short, imperative commit subject such as `Fix ...` or `Add ...`.
2. Explain the behavior changed and list the validation commands you ran. For a bug fix, include reproduction steps and environment details.
3. For UI changes, include a screenshot or a short note describing what you checked in the browser.
4. Link the relevant issue when there is one, then open the pull request for review.

See the repository's [MIT license](LICENSE) for license terms.
