// SPDX-License-Identifier: LGPL-3.0-only WITH LGPL-3.0-linking-exception
//! Metric-aware CRT column grid.
//!
//! Each CRT keeps a **fixed column grid** derived from the default font
//! (Hurmit) at the panel's design size. The rendered size for any font is
//! `size = column_px / advance_em`, so switching to OpenDyslexicM shrinks the
//! text (about 16 %) instead of reflowing it. Below the floor (12 px desktop,
//! 13 px phone) the size is clamped and the grid loses columns; long labels
//! then truncate with `…` instead of shrinking further.

use crate::Rect;
use mu_console_theme::metrics::{CrtFontMetrics, HURMIT};

/// Line height as a multiple of the design size.
pub const LINE_FACTOR: f64 = 1.3;

/// Per-CRT grid parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridSpec {
    /// Design text size in px (with the default font).
    pub design_px: f64,
    /// Legibility floor in px.
    pub floor_px: f64,
    /// Horizontal text padding inside the screen.
    pub pad_x: f64,
    /// Vertical text padding inside the screen.
    pub pad_y: f64,
}

/// Solved text grid of one CRT.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrtGrid {
    /// Character columns per row (indexes text, so `usize`).
    pub cols: usize,
    /// Text rows per page.
    pub rows: usize,
    /// Rendered font size in px.
    pub font_px: f64,
    /// Column pitch in px (`font_px * advance_em`).
    pub column_px: f64,
    /// Row pitch in px.
    pub row_px: f64,
    /// Text area inside the screen.
    pub text_area: Rect,
    /// The floor clamped the size: the grid has fewer columns than the
    /// design grid and labels must be truncated.
    pub clamped: bool,
}

impl CrtGrid {
    /// Width of a full row of text in px.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn text_width(&self) -> f64 {
        self.cols as f64 * self.column_px
    }

    /// Height of a full page of rows in px.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn text_height(&self) -> f64 {
        self.rows as f64 * self.row_px
    }

    /// Truncate `label` to this grid's width.
    #[must_use]
    pub fn fit(&self, label: &str) -> String {
        truncate(label, self.cols)
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn floor_count(space: f64, pitch: f64) -> usize {
    if pitch <= 0.0 || !space.is_finite() || space <= 0.0 {
        return 0;
    }
    // Small epsilon so exact fits are not lost to rounding.
    (space / pitch + 1e-9).floor() as usize
}

/// Solve the grid for `screen` (the CRT glass rect) and `font`.
#[must_use]
pub fn solve_grid(screen: Rect, spec: GridSpec, font: &CrtFontMetrics) -> CrtGrid {
    let text_area = screen.inset(spec.pad_x, spec.pad_y, spec.pad_x, spec.pad_y);
    let design_column = spec.design_px * HURMIT.advance_em;
    let design_cols = floor_count(text_area.w, design_column);
    let mut font_px = design_column / font.advance_em;
    let mut cols = design_cols;
    let mut clamped = false;
    if font_px < spec.floor_px {
        font_px = spec.floor_px;
        cols = floor_count(text_area.w, font_px * font.advance_em);
        clamped = true;
    }
    let row_px = (spec.design_px * LINE_FACTOR).max(font_px * LINE_FACTOR);
    let rows = floor_count(text_area.h, row_px);
    CrtGrid {
        cols,
        rows,
        font_px,
        column_px: font_px * font.advance_em,
        row_px,
        text_area,
        clamped,
    }
}

/// Truncate to `cols` characters, ending with `…` when cut.
#[must_use]
pub fn truncate(label: &str, cols: usize) -> String {
    let n = label.chars().count();
    if n <= cols {
        return label.to_owned();
    }
    if cols == 0 {
        return String::new();
    }
    let mut s: String = label.chars().take(cols - 1).collect();
    s.push('…');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use mu_console_theme::metrics::OPEN_DYSLEXIC;

    const SPEC: GridSpec = GridSpec {
        design_px: 16.0,
        floor_px: 12.0,
        pad_x: 10.0,
        pad_y: 8.0,
    };

    #[test]
    fn same_grid_for_both_fonts_above_floor() {
        let screen = Rect::new(0.0, 0.0, 854.0, 484.0);
        let h = solve_grid(screen, SPEC, &HURMIT);
        let d = solve_grid(screen, SPEC, &OPEN_DYSLEXIC);
        assert_eq!(h.cols, d.cols);
        assert_eq!(h.rows, d.rows);
        assert!(!d.clamped);
        assert!((h.font_px - 16.0).abs() < 1e-9);
        assert!((d.font_px - 16.0 * 0.618 / 0.732).abs() < 1e-9);
        assert!((h.column_px - d.column_px).abs() < 1e-9);
        assert!(h.text_width() <= h.text_area.w + 1e-9);
    }

    #[test]
    fn floor_clamps_and_drops_columns() {
        let spec = GridSpec {
            design_px: 14.0,
            ..SPEC
        };
        let screen = Rect::new(0.0, 0.0, 300.0, 100.0);
        let d = solve_grid(screen, spec, &OPEN_DYSLEXIC);
        assert!(d.clamped);
        assert!((d.font_px - 12.0).abs() < 1e-9);
        let h = solve_grid(screen, spec, &HURMIT);
        assert!(d.cols < h.cols);
        assert!(d.text_width() <= d.text_area.w + 1e-9);
    }

    #[test]
    fn truncation() {
        assert_eq!(truncate("MODEL-RUNNER", 20), "MODEL-RUNNER");
        assert_eq!(truncate("MODEL-RUNNER", 12), "MODEL-RUNNER");
        assert_eq!(truncate("MODEL-RUNNER", 6), "MODEL…");
        assert_eq!(truncate("MODEL-RUNNER", 1), "…");
        assert_eq!(truncate("MODEL-RUNNER", 0), "");
        assert_eq!(truncate("feat/äöü-tiles", 5), "feat…");
    }

    #[test]
    fn degenerate_screen_has_empty_grid() {
        let g = solve_grid(Rect::new(0.0, 0.0, 10.0, 10.0), SPEC, &HURMIT);
        assert_eq!((g.cols, g.rows), (0, 0));
        assert_eq!(g.fit("abc"), "");
    }
}
