// SPDX-License-Identifier: LGPL-3.0-only WITH LGPL-3.0-linking-exception
//! Pure layout solver for MuConsole.
//!
//! `solve(viewport, safe_area, area, font) -> Frames` places every top-level
//! region of the console so that **nothing overflows and the page never
//! scrolls**: the frames exactly tile the usable canvas, and the active
//! sub-view canvas gets a metric-aware column grid with at least one row and
//! one column.
//!
//! The crate has no browser dependencies; it is unit tested natively for
//! every target viewport and both CRT fonts.
#![forbid(unsafe_code)]

pub mod grid;
pub mod trainway;

pub use grid::{solve_grid, truncate, CrtGrid, GridSpec};
pub use mu_console_protocol::Area;
pub use mu_console_theme::instruments::Rect;
pub use trainway::{allocate_lanes, AllocatedLane, TrainwayLayout};

use mu_console_theme::metrics::{CrtFontMetrics, CRT_FLOOR_DESKTOP_PX, CRT_FLOOR_PHONE_PX};

/// CSS-px viewport (from `visualViewport`, never hard-coded).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
}

impl Viewport {
    #[must_use]
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    /// Map `window.visualViewport` (`width`, `height`, `scale`) to the
    /// layout canvas.
    #[must_use]
    pub fn from_visual_viewport(width: f64, height: f64, scale: f64) -> Self {
        let s = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        Self::new((width * s).max(0.0), (height * s).max(0.0))
    }
}

/// `env(safe-area-inset-*)` values in CSS px.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SafeArea {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl SafeArea {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };
}

/// Breakpoint class (decided here, not in CSS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Breakpoint {
    /// w >= 1280 and h >= 760: 80 px switch rail, C2 top bar, 36 px SubViewBar,
    /// full-canvas ActiveSubViewCanvas.
    Wide,
    /// Everything between: 80 px rail, C2 top bar, 36 px SubViewBar,
    /// full-canvas ActiveSubViewCanvas.
    Medium,
    /// h < 480: 72 px rail, compact bars, full-canvas ActiveSubViewCanvas.
    CompactLandscape,
    /// w < 520: bottom selector strip, compact bars, full-canvas ActiveSubViewCanvas.
    Phone,
}

/// Classify a usable canvas size.
#[must_use]
pub fn classify(width: f64, height: f64) -> Breakpoint {
    if width < 520.0 {
        Breakpoint::Phone
    } else if height < 480.0 {
        Breakpoint::CompactLandscape
    } else if width >= 1280.0 && height >= 760.0 {
        Breakpoint::Wide
    } else {
        Breakpoint::Medium
    }
}

/// Dashboard panel kind corresponding to the active top-level area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelKind {
    Trainway,
    Delivery,
    Registry,
    Observe,
    Fleet,
}

impl PanelKind {
    #[must_use]
    pub const fn for_area(area: Area) -> Self {
        match area {
            Area::Ci => Self::Trainway,
            Area::Cd => Self::Delivery,
            Area::Registry => Self::Registry,
            Area::Observe => Self::Observe,
            Area::Fleet => Self::Fleet,
        }
    }
}

/// One dashboard panel: tungsten plate, CRT glass and its text grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub kind: PanelKind,
    /// Panels with the same page are visible together.
    pub page: u32,
    pub plate: Rect,
    pub crt: Rect,
    pub grid: CrtGrid,
}

/// One twin-CRT scorecard module (used inside the `CI -> Scorecards` sub-view).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Card {
    pub plate: Rect,
    pub ci_crt: Rect,
    pub cd_crt: Rect,
    pub ci_grid: CrtGrid,
    pub cd_grid: CrtGrid,
}

/// Legacy carousel container retained for `Card` grid helpers.
#[derive(Debug, Clone, PartialEq)]
pub struct Carousel {
    pub cards: Vec<Card>,
    pub switch: Option<Rect>,
    pub dots: Rect,
}

impl Carousel {
    /// Bounding box of the cards, switch and dots.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        let mut rects: Vec<Rect> = self.cards.iter().map(|c| c.plate).collect();
        rects.extend(self.switch);
        rects.push(self.dots);
        union(&rects)
    }
}

/// Solved frames for one viewport.
///
/// Following the usability-first single-sub-view architecture, every viewport
/// consists of 4 non-overlapping top-level regions:
/// 1. `nav` (`C1` left rail on desktop/landscape, bottom bar on phone)
/// 2. `top_bar` (`C2` breadcrumb + compact Repo/Submodule selector + status + font switch)
/// 3. `subview_bar` (`36px` horizontal sub-view tab bar)
/// 4. `active_canvas` (`100%` of remaining height dedicated to the active sub-view)
#[derive(Debug, Clone, PartialEq)]
pub struct Frames {
    pub breakpoint: Breakpoint,
    /// Usable canvas (viewport minus safe areas).
    pub canvas: Rect,
    /// Area navigation: switch rail (desktop) or bottom selector strip.
    pub nav: Rect,
    /// Breadcrumb bar (`C2`).
    pub top_bar: Rect,
    /// Horizontal sub-view tab strip (`36px` tall) directly below `top_bar`.
    pub subview_bar: Rect,
    /// Full-canvas container for the single active sub-view.
    pub active_canvas: Rect,
    /// Deprecated compound-view carousel (always `None` in single-sub-view layout).
    pub carousel: Option<Carousel>,
    /// Deprecated standalone rotary module (always `None`; replaced by `C2` repo selector).
    pub rotary: Option<Rect>,
    /// Single full-canvas panel matching `active_canvas`.
    pub panels: Vec<Panel>,
    /// Number of dashboard pages (always `1` in single-sub-view layout).
    pub pages: u32,
}

impl Frames {
    /// Top-level regions that tile the canvas without overlapping.
    #[must_use]
    pub fn regions(&self) -> Vec<Rect> {
        vec![self.nav, self.top_bar, self.subview_bar, self.active_canvas]
    }

    /// Panels on `page`.
    pub fn page(&self, page: u32) -> impl Iterator<Item = &Panel> {
        self.panels.iter().filter(move |p| p.page == page)
    }
}

/// Outer margin around the desk.
pub const MARGIN: f64 = 6.0;
/// Gap between the rail and the content column.
pub const RAIL_GAP: f64 = 8.0;
/// Desktop rail plate width.
pub const RAIL_W: f64 = 80.0;
/// Compact-landscape rail plate width.
pub const RAIL_W_COMPACT: f64 = 72.0;
/// Breadcrumb bar height (desktop / compact / phone).
pub const TOP_BAR_H: f64 = 48.0;
pub const TOP_BAR_H_COMPACT: f64 = 40.0;
pub const TOP_BAR_H_PHONE: f64 = 44.0;
/// Sub-view tab bar height (`36px` across viewports).
pub const SUBVIEW_BAR_H: f64 = 36.0;
/// Vertical gap between `top_bar`, `subview_bar`, and `active_canvas`.
pub const SUBVIEW_GAP: f64 = 6.0;
pub const SUBVIEW_GAP_COMPACT: f64 = 4.0;
/// Scorecard height inside `CiSubView::Scorecards`.
pub const CARD_H: f64 = 184.0;
pub const CARD_H_MIN: f64 = 132.0;
pub const CARD_GAP: f64 = 12.0;
pub const CARD_MIN_W: f64 = 220.0;
/// Phone bottom selector strip height and gap.
pub const PHONE_NAV_H: f64 = 60.0;
pub const PHONE_GAP: f64 = 6.0;

const PANEL_GRID: GridSpec = GridSpec {
    design_px: 16.0,
    floor_px: CRT_FLOOR_DESKTOP_PX,
    pad_x: 10.0,
    pad_y: 8.0,
};
const CARD_GRID: GridSpec = GridSpec {
    design_px: 14.0,
    floor_px: CRT_FLOOR_DESKTOP_PX,
    pad_x: 6.0,
    pad_y: 6.0,
};

fn phone_grid(spec: GridSpec, design_px: f64) -> GridSpec {
    GridSpec {
        design_px,
        floor_px: CRT_FLOOR_PHONE_PX,
        ..spec
    }
}

/// Bounding box of `rects` (empty input gives a zero rect).
#[must_use]
pub fn union(rects: &[Rect]) -> Rect {
    let Some(first) = rects.first() else {
        return Rect::default();
    };
    let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.right(), first.bottom());
    for r in &rects[1..] {
        x0 = x0.min(r.x);
        y0 = y0.min(r.y);
        x1 = x1.max(r.right());
        y1 = y1.max(r.bottom());
    }
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

fn card(
    plate: Rect,
    top: f64,
    bottom: f64,
    side: f64,
    gap: f64,
    spec: GridSpec,
    font: &CrtFontMetrics,
) -> Card {
    let crt_w = ((plate.w - 2.0 * side - gap) / 2.0).max(0.0);
    let crt_h = (plate.h - top - bottom).max(0.0);
    let ci_crt = Rect::new(plate.x + side, plate.y + top, crt_w, crt_h);
    let cd_crt = Rect::new(plate.right() - side - crt_w, plate.y + top, crt_w, crt_h);
    Card {
        plate,
        ci_crt,
        cd_crt,
        ci_grid: solve_grid(ci_crt, spec, font),
        cd_grid: solve_grid(cd_crt, spec, font),
    }
}

/// Desktop card: header strip on top, twin CRTs below.
#[must_use]
pub fn desktop_card(plate: Rect, font: &CrtFontMetrics) -> Card {
    card(plate, 38.0, 14.0, 14.0, 10.0, CARD_GRID, font)
}

/// Compute the dynamic `(cols, rows)` grid capacity of twin-CRT scorecards
/// that fit inside `area` without horizontal truncation or vertical overflow.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn scorecard_grid_dims(area: Rect, bp: Breakpoint, _font: &CrtFontMetrics) -> (usize, usize) {
    let pad = 12.0;
    let inner = area.inset(pad, pad, pad, pad);
    let cols = match bp {
        Breakpoint::Phone => 1,
        Breakpoint::CompactLandscape => {
            (((inner.w + CARD_GAP) / (280.0 + CARD_GAP)).floor() as usize).clamp(1, 3)
        }
        Breakpoint::Medium => {
            (((inner.w + CARD_GAP) / (360.0 + CARD_GAP)).floor() as usize).clamp(1, 3)
        }
        Breakpoint::Wide => {
            if inner.w >= 1100.0 {
                (((inner.w + CARD_GAP) / (415.0 + CARD_GAP)).floor() as usize).clamp(3, 6)
            } else {
                2
            }
        }
    };
    let target_row_h = match bp {
        Breakpoint::CompactLandscape => 122.0,
        Breakpoint::Phone => 142.0,
        Breakpoint::Medium | Breakpoint::Wide => 166.0,
    };
    let rows = (((inner.h + CARD_GAP) / (target_row_h + CARD_GAP)).floor() as usize).clamp(1, 6);
    (cols.max(1), rows.max(1))
}

/// Compute the dynamic number of repository scorecards per page (`cols * rows`)
/// for `area`, `bp`, and `font`.
#[must_use]
pub fn scorecard_page_capacity(area: Rect, bp: Breakpoint, font: &CrtFontMetrics) -> usize {
    let (cols, rows) = scorecard_grid_dims(area, bp, font);
    (cols * rows).max(1)
}

/// Compute the dynamic number of visible branch sidings per Trainway page
/// for `screen_rect`, `bp`, and `grid_rows`.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn trainway_siding_capacity(screen_rect: Rect, bp: Breakpoint, grid_rows: usize) -> usize {
    let max_by_grid = grid_rows.saturating_sub(4).max(2);
    let by_height = match bp {
        Breakpoint::Phone => 3,
        Breakpoint::CompactLandscape => 4,
        Breakpoint::Medium | Breakpoint::Wide => {
            (((screen_rect.h - 100.0).max(140.0) / 70.0).floor() as usize).clamp(4, 14)
        }
    };
    by_height.min(max_by_grid).max(2)
}

/// Compute a responsive grid of twin-CRT repository scorecard plates inside `area`.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn scorecard_cards(
    area: Rect,
    bp: Breakpoint,
    count: usize,
    font: &CrtFontMetrics,
) -> Vec<Card> {
    if count == 0 {
        return Vec::new();
    }
    let (grid_cols, grid_rows) = scorecard_grid_dims(area, bp, font);
    let cols = grid_cols.min(count).max(1);
    let rows = count.div_ceil(cols).max(1);
    let layout_rows = rows.max(grid_rows);
    let pad = 12.0;
    let inner = area.inset(pad, pad, pad, pad);
    let card_w = ((inner.w - CARD_GAP * (grid_cols.min(count).saturating_sub(1)) as f64)
        / cols as f64)
        .floor()
        .max(120.0);
    let min_h = match bp {
        Breakpoint::CompactLandscape => 118.0,
        _ => CARD_H_MIN,
    };
    let avail_h = ((inner.h - CARD_GAP * (layout_rows.saturating_sub(1)) as f64)
        / layout_rows as f64)
        .floor()
        .clamp(min_h, CARD_H);
    let spec = match bp {
        Breakpoint::Wide | Breakpoint::Medium => CARD_GRID,
        Breakpoint::CompactLandscape | Breakpoint::Phone => phone_grid(CARD_GRID, 14.0),
    };
    let (top_inset, bottom_inset) = if avail_h < 142.0 {
        (28.0, 8.0)
    } else {
        (34.0, 10.0)
    };
    (0..count)
        .map(|idx| {
            let col = idx % cols;
            let row = idx / cols;
            let x = inner.x + col as f64 * (card_w + CARD_GAP);
            let y = inner.y + row as f64 * (avail_h + CARD_GAP);
            card(
                Rect::new(x, y, card_w, avail_h),
                top_inset,
                bottom_inset,
                10.0,
                8.0,
                spec,
                font,
            )
        })
        .collect()
}

fn panel(
    kind: PanelKind,
    page: u32,
    plate: Rect,
    insets: (f64, f64, f64, f64),
    spec: GridSpec,
    font: &CrtFontMetrics,
) -> Panel {
    let crt = plate.inset(insets.0, insets.1, insets.2, insets.3);
    Panel {
        kind,
        page,
        plate,
        crt,
        grid: solve_grid(crt, spec, font),
    }
}

/// Plate -> CRT insets: header strip with title, filters and actions.
const CANVAS_INSETS: (f64, f64, f64, f64) = (18.0, 52.0, 18.0, 18.0);
const COMPACT_INSETS: (f64, f64, f64, f64) = (12.0, 40.0, 12.0, 12.0);
const PHONE_INSETS: (f64, f64, f64, f64) = (10.0, 36.0, 10.0, 10.0);

fn solve_desktop(c: Rect, bp: Breakpoint, area: Area, font: &CrtFontMetrics) -> Frames {
    let nav = Rect::new(c.x + MARGIN, c.y + MARGIN, RAIL_W, c.h - 2.0 * MARGIN);
    let x0 = nav.right() + RAIL_GAP;
    let x1 = c.right() - MARGIN;
    let cw = (x1 - x0).max(0.0);
    let top_bar = Rect::new(x0, c.y + MARGIN, cw, TOP_BAR_H);
    let subview_bar = Rect::new(x0, top_bar.bottom() + SUBVIEW_GAP, cw, SUBVIEW_BAR_H);
    let dash_y = subview_bar.bottom() + SUBVIEW_GAP;
    let dash_h = (c.bottom() - MARGIN - dash_y).max(0.0);
    let active_canvas = Rect::new(x0, dash_y, cw, dash_h);
    let panels = vec![panel(
        PanelKind::for_area(area),
        0,
        active_canvas,
        CANVAS_INSETS,
        PANEL_GRID,
        font,
    )];
    Frames {
        breakpoint: bp,
        canvas: c,
        nav,
        top_bar,
        subview_bar,
        active_canvas,
        carousel: None,
        rotary: None,
        panels,
        pages: 1,
    }
}

fn solve_compact(c: Rect, area: Area, font: &CrtFontMetrics) -> Frames {
    let nav = Rect::new(
        c.x + MARGIN,
        c.y + MARGIN,
        RAIL_W_COMPACT,
        c.h - 2.0 * MARGIN,
    );
    let x0 = nav.right() + RAIL_GAP;
    let cw = (c.right() - MARGIN - x0).max(0.0);
    let top_bar = Rect::new(x0, c.y + MARGIN, cw, TOP_BAR_H_COMPACT);
    let subview_bar = Rect::new(
        x0,
        top_bar.bottom() + SUBVIEW_GAP_COMPACT,
        cw,
        SUBVIEW_BAR_H,
    );
    let dash_y = subview_bar.bottom() + SUBVIEW_GAP_COMPACT;
    let dash_h = (c.bottom() - MARGIN - dash_y).max(0.0);
    let active_canvas = Rect::new(x0, dash_y, cw, dash_h);
    let panels = vec![panel(
        PanelKind::for_area(area),
        0,
        active_canvas,
        COMPACT_INSETS,
        phone_grid(PANEL_GRID, 15.0),
        font,
    )];
    Frames {
        breakpoint: Breakpoint::CompactLandscape,
        canvas: c,
        nav,
        top_bar,
        subview_bar,
        active_canvas,
        carousel: None,
        rotary: None,
        panels,
        pages: 1,
    }
}

fn solve_phone(c: Rect, area: Area, font: &CrtFontMetrics) -> Frames {
    let x0 = c.x + MARGIN;
    let cw = (c.w - 2.0 * MARGIN).max(0.0);
    let top_bar = Rect::new(x0, c.y + MARGIN, cw, TOP_BAR_H_PHONE);
    let subview_bar = Rect::new(
        x0,
        top_bar.bottom() + SUBVIEW_GAP_COMPACT,
        cw,
        SUBVIEW_BAR_H,
    );
    let nav = Rect::new(x0, c.bottom() - MARGIN - PHONE_NAV_H, cw, PHONE_NAV_H);
    let dash_y = subview_bar.bottom() + SUBVIEW_GAP_COMPACT;
    let dash_h = (nav.y - PHONE_GAP - dash_y).max(0.0);
    let active_canvas = Rect::new(x0, dash_y, cw, dash_h);
    let panels = vec![panel(
        PanelKind::for_area(area),
        0,
        active_canvas,
        PHONE_INSETS,
        phone_grid(PANEL_GRID, 16.0),
        font,
    )];
    Frames {
        breakpoint: Breakpoint::Phone,
        canvas: c,
        nav,
        top_bar,
        subview_bar,
        active_canvas,
        carousel: None,
        rotary: None,
        panels,
        pages: 1,
    }
}

/// Solve the frames for a viewport, safe-area insets, area and CRT font.
#[must_use]
pub fn solve(viewport: Viewport, safe: SafeArea, area: Area, font: &CrtFontMetrics) -> Frames {
    let canvas = Rect::new(
        safe.left,
        safe.top,
        (viewport.width - safe.left - safe.right).max(0.0),
        (viewport.height - safe.top - safe.bottom).max(0.0),
    );
    match classify(canvas.w, canvas.h) {
        bp @ (Breakpoint::Wide | Breakpoint::Medium) => solve_desktop(canvas, bp, area, font),
        Breakpoint::CompactLandscape => solve_compact(canvas, area, font),
        Breakpoint::Phone => solve_phone(canvas, area, font),
    }
}

/// Solve for a full route (only the area affects the frames).
#[must_use]
pub fn solve_route(
    viewport: Viewport,
    safe: SafeArea,
    route: &mu_console_protocol::Route,
    font: &CrtFontMetrics,
) -> Frames {
    solve(viewport, safe, route.area, font)
}

#[cfg(test)]
mod tests;
