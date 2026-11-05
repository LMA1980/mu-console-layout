# mu-console-layout

Pure layout solver for MuConsole: breakpoints, frame tiling, and metric-aware CRT column grids with zero page scroll.

## Architecture

- **Zero-Scroll Viewport Tiling**: Every top-level region (`nav`, `top_bar`, `subview_bar`, `active_canvas`) tiles the usable canvas without overlap or scroll leaks across all breakpoints (`Wide`, `Medium`, `CompactLandscape`, `Phone`).
- **Metric-Aware CRT Column Grid**: Fixed character columns derived from the Hurmit design size. Font switches (such as OpenDyslexic) scale down text rather than reflowing layout columns, bounded by design floors (12px desktop, 13px phone).
- **Deterministic Trainway Allocator**: Branch sidings are allocated stable lanes (`1..`). Existing branches never shift or permute lanes when other branches are added, merged, or deleted. Includes deterministic `YARD +N` pagination.

## Testing & Verification

```bash
cargo test --all-targets
cargo check --target wasm32-unknown-unknown
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## License

`mu-console-layout` is licensed under the GNU Lesser General Public License version 3.0 with the LGPL-3.0 Linking Exception (`LGPL-3.0-only WITH LGPL-3.0-linking-exception`). See [LICENSE](LICENSE), [COPYING](COPYING), and [LINKING-EXCEPTION](LINKING-EXCEPTION).
