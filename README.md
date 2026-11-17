# mu-console-layout

Pure layout solver for [`mu-console`](https://github.com/LMA1980/mu-console): breakpoints, frame tiling, and metric-aware CRT column grids.

## Architecture

- **Zero-Scroll Viewport Tiling (`WireframeSkin`)**: Governs the fixed-viewport `WireframeSkin` (`html[data-skin="wireframe"]`), tiling every top-level region (`nav`, `top_bar`, `subview_bar`, `active_canvas`) across the usable canvas without overlap or scroll leaks across all breakpoints (`Wide`, `Medium`, `CompactLandscape`, `Phone`). The default `HtmlCoreSkin` (`html[data-skin="html"]`) uses natural vertical page scrolling instead of fixed viewport framing.
- **Metric-Aware CRT Column Grid**: Fixed character columns derived from the Hurmit design size via [`mu-console-theme`](https://github.com/LMA1980/mu-console-theme) metrics. Font switches (such as OpenDyslexic) scale down text rather than reflowing layout columns, bounded by design floors (12px desktop, 13px phone).
- **Deterministic Trainway Allocator**: Branch sidings are allocated stable lanes (`1..`). Existing branches never shift or permute lanes when other branches are added, merged, or deleted. Includes deterministic `YARD +N` pagination.

## Ecosystem Integration

- **Parent Workspace**: [`mu-console`](https://github.com/LMA1980/mu-console)
- **Companion Crates**: [`mu-console-protocol`](https://github.com/LMA1980/mu-console-protocol), [`mu-console-theme`](https://github.com/LMA1980/mu-console-theme), [`mu-console-ui`](https://github.com/LMA1980/mu-console-ui)

## Testing & Verification

```bash
cargo fmt --all -- --check
./scripts/format_docs.sh --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --target wasm32-unknown-unknown
MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly miri test --lib
```

## License

`mu-console-layout` is licensed under the GNU Lesser General Public License version 3.0 with the LGPL-3.0 Linking Exception (`LGPL-3.0-only WITH LGPL-3.0-linking-exception`). See [LICENSE](LICENSE), [COPYING](COPYING), and [LINKING-EXCEPTION](LINKING-EXCEPTION).
