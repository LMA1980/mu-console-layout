______________________________________________________________________

## name: mu-console-layout description: "Pure deterministic layout solver for MuConsole: breakpoints, frame tiling, and metric-aware CRT column grids with zero page scroll."

# `mu-console-layout` Pure Layout Solver

Use this skill when developing, testing, or calculating UI layout frames, breakpoints, safe-area offsets, CRT column grids, and Trainway siding allocations across the MuConsole terminal architecture.

## 1. Layout Principles & Invariants

- **Zero Page Scroll**: The console layout tiles the usable viewport exactly (`canvas = viewport - safe_area`). The page never scrolls; every region is strictly bounded.
- **Top-Level Regions**:
  - `nav`: C1 left switch rail on wide/medium/compact screens, bottom strip on phone screens.
  - `top_bar`: C2 breadcrumbs, submodule/repo dropdown, and font switch.
  - `subview_bar`: 36px horizontal sub-view selector strip.
  - `active_canvas`: 100% of remaining viewport dedicated to the single active sub-view.
- **Metric-Aware CRT Column Grid**: Derives fixed character columns from the Hurmit design size. Font switches (e.g., OpenDyslexic) shrink font size rather than reflowing columns. Clamps at floor limits (12px desktop, 13px phone) to guarantee readability.
- **Trainway Siding Allocation**: Stable lane indexing (`1..` for sidings, `0` reserved for main line). Existing branches retain assigned lanes when unrelated branches are added or deleted. Lowest free lane is claimed by new branches.

## 2. Public APIs

- `solve(viewport, safe_area, area, font) -> Frames`: Solves non-overlapping frames for the given canvas and CRT font metrics.
- `solve_route(viewport, safe_area, route, font) -> Frames`: Helper routing directly from a protocol `Route`.
- `solve_grid(screen, spec, font) -> CrtGrid`: Calculates character columns, rows, font size, and pitch for CRT glass text.
- `allocate_lanes(topology, previous_lanes, max_visible_sidings, yard_page) -> TrainwayLayout`: Pure deterministic Trainway lane layout with `YARD +N` pagination.

## 3. Verification Commands

```bash
cargo test --all-targets
cargo check --target wasm32-unknown-unknown
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```
