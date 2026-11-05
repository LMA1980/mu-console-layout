// SPDX-License-Identifier: LGPL-3.0-only WITH LGPL-3.0-linking-exception
//! Zero-overflow / no-scroll tests at every target viewport, for both fonts.

use super::*;
use mu_console_theme::metrics::{CRT_FONTS, HURMIT, OPEN_DYSLEXIC};

const EPS: f64 = 0.01;
const AREAS: [Area; 5] = Area::ALL;

/// Design targets: (width, height, expected breakpoint).
const TARGETS: [(f64, f64, Breakpoint); 6] = [
    (1512.0, 860.0, Breakpoint::Wide),
    (1512.0, 982.0, Breakpoint::Wide),
    (1800.0, 1040.0, Breakpoint::Wide),
    (402.0, 720.0, Breakpoint::Phone),
    (402.0, 778.0, Breakpoint::Phone),
    (874.0, 340.0, Breakpoint::CompactLandscape),
];

fn inside(inner: &Rect, outer: &Rect) -> bool {
    inner.x >= outer.x - EPS
        && inner.y >= outer.y - EPS
        && inner.right() <= outer.right() + EPS
        && inner.bottom() <= outer.bottom() + EPS
}

fn overlap(a: &Rect, b: &Rect) -> bool {
    a.x < b.right() - EPS
        && b.x < a.right() - EPS
        && a.y < b.bottom() - EPS
        && b.y < a.bottom() - EPS
}

fn assert_grid(g: &CrtGrid, screen: &Rect, floor: f64, what: &str) {
    assert!(g.cols >= 1, "{what}: no columns");
    assert!(g.rows >= 1, "{what}: no rows");
    assert!(
        g.font_px >= floor - EPS,
        "{what}: {} px below floor {floor}",
        g.font_px
    );
    assert!(
        g.text_width() <= g.text_area.w + EPS,
        "{what}: text overflows width"
    );
    assert!(
        g.text_height() <= g.text_area.h + EPS,
        "{what}: text overflows height"
    );
    assert!(
        inside(&g.text_area, screen),
        "{what}: text area outside screen"
    );
}

/// The invariants behind "nothing overflows and nothing scrolls".
fn assert_no_overflow(f: &Frames, label: &str) {
    let c = f.canvas;
    let regions = f.regions();
    for r in &regions {
        assert!(r.w > 0.0 && r.h > 0.0, "{label}: empty region {r:?}");
        assert!(inside(r, &c), "{label}: {r:?} outside canvas {c:?}");
    }
    for (i, a) in regions.iter().enumerate() {
        for b in &regions[i + 1..] {
            assert!(!overlap(a, b), "{label}: {a:?} overlaps {b:?}");
        }
    }
    let top = regions.iter().map(|r| r.y).fold(f64::INFINITY, f64::min);
    let bottom = regions.iter().map(Rect::bottom).fold(0.0, f64::max);
    assert!((top - (c.y + MARGIN)).abs() < 0.5, "{label}: top {top}");
    assert!(
        (bottom - (c.bottom() - MARGIN)).abs() < 0.5,
        "{label}: bottom {bottom}"
    );
    let right = regions.iter().map(Rect::right).fold(0.0, f64::max);
    assert!(
        (right - (c.right() - MARGIN)).abs() < 0.5,
        "{label}: right {right}"
    );

    let floor = match f.breakpoint {
        Breakpoint::Wide | Breakpoint::Medium => CRT_FLOOR_DESKTOP_PX,
        Breakpoint::CompactLandscape | Breakpoint::Phone => CRT_FLOOR_PHONE_PX,
    };
    assert_eq!(f.pages, 1, "{label}: single active sub-view page");
    assert_eq!(f.panels.len(), 1, "{label}: single full-canvas panel");
    assert!(
        (f.subview_bar.h - SUBVIEW_BAR_H).abs() < EPS,
        "{label}: subview_bar height"
    );
    for p in &f.panels {
        assert!(p.page < f.pages, "{label}: page out of range");
        assert_eq!(
            p.plate, f.active_canvas,
            "{label}: panel plate matches active_canvas"
        );
        assert!(
            inside(&p.crt, &p.plate),
            "{label}: {:?} CRT outside plate",
            p.kind
        );
        assert_grid(&p.grid, &p.crt, floor, &format!("{label} {:?}", p.kind));
    }
}

#[test]
fn target_viewports_never_overflow() {
    for (w, h, bp) in TARGETS {
        for font in &CRT_FONTS {
            for area in AREAS {
                let f = solve(Viewport::new(w, h), SafeArea::ZERO, area, font);
                let label = format!("{w}x{h} {} {area:?}", font.family);
                assert_eq!(f.breakpoint, bp, "{label}");
                assert_no_overflow(&f, &label);
            }
        }
    }
}

#[test]
fn wide_matches_single_subview_geometry() {
    let f = solve(
        Viewport::new(1512.0, 860.0),
        SafeArea::ZERO,
        Area::Ci,
        &HURMIT,
    );
    assert_eq!(f.nav, Rect::new(6.0, 6.0, 80.0, 848.0));
    assert_eq!(f.top_bar, Rect::new(94.0, 6.0, 1412.0, 48.0));
    assert_eq!(f.subview_bar, Rect::new(94.0, 60.0, 1412.0, 36.0));
    assert_eq!(f.active_canvas, Rect::new(94.0, 102.0, 1412.0, 752.0));
    assert!(f.carousel.is_none());
    assert!(f.rotary.is_none());
    assert_eq!(f.panels.len(), 1);
    assert_eq!(f.panels[0].kind, PanelKind::Trainway);
    assert_eq!(f.panels[0].plate, f.active_canvas);
}

#[test]
fn scorecard_grid_populates_non_overlapping_cards() {
    let f = solve(
        Viewport::new(1512.0, 860.0),
        SafeArea::ZERO,
        Area::Ci,
        &HURMIT,
    );
    let cards = scorecard_cards(f.panels[0].crt, f.breakpoint, 6, &HURMIT);
    assert_eq!(cards.len(), 6);
    for (i, a) in cards.iter().enumerate() {
        assert!(inside(&a.plate, &f.panels[0].crt));
        assert!(inside(&a.ci_crt, &a.plate));
        assert!(inside(&a.cd_crt, &a.plate));
        assert!(!overlap(&a.ci_crt, &a.cd_crt));
        for b in &cards[i + 1..] {
            assert!(!overlap(&a.plate, &b.plate));
        }
    }
}

#[test]
fn font_switch_keeps_the_column_grid() {
    for (w, h, _) in TARGETS {
        let a = solve(Viewport::new(w, h), SafeArea::ZERO, Area::Ci, &HURMIT);
        let b = solve(
            Viewport::new(w, h),
            SafeArea::ZERO,
            Area::Ci,
            &OPEN_DYSLEXIC,
        );
        assert_eq!(
            a.regions(),
            b.regions(),
            "{w}x{h}: frames move with the font"
        );
        for (pa, pb) in a.panels.iter().zip(&b.panels) {
            assert_eq!(pa.grid.rows, pb.grid.rows, "{w}x{h} {:?} rows", pa.kind);
            if !pb.grid.clamped {
                assert_eq!(pa.grid.cols, pb.grid.cols, "{w}x{h} {:?} cols", pa.kind);
                let shrink = pb.grid.font_px / pa.grid.font_px;
                assert!((shrink - 0.618 / 0.732).abs() < 1e-9);
            }
        }
    }
}

#[test]
fn desktop_panels_keep_design_size() {
    let f = solve(
        Viewport::new(1512.0, 860.0),
        SafeArea::ZERO,
        Area::Ci,
        &OPEN_DYSLEXIC,
    );
    for p in &f.panels {
        assert!(!p.grid.clamped, "{:?} clamped at desktop size", p.kind);
        assert!(p.grid.cols >= 80, "{:?} only {} cols", p.kind, p.grid.cols);
        assert!(p.grid.rows >= 24, "{:?} only {} rows", p.kind, p.grid.rows);
    }
}

#[test]
fn phone_layout_and_safe_areas() {
    let f = solve(
        Viewport::new(402.0, 720.0),
        SafeArea::ZERO,
        Area::Ci,
        &HURMIT,
    );
    assert_eq!(f.pages, 1);
    assert!(f.rotary.is_none());
    assert!(f.carousel.is_none());
    assert!((f.nav.h - PHONE_NAV_H).abs() < EPS);
    assert!((f.subview_bar.h - SUBVIEW_BAR_H).abs() < EPS);

    let safe = SafeArea {
        top: 62.0,
        bottom: 34.0,
        ..SafeArea::ZERO
    };
    let pwa = solve(Viewport::new(402.0, 874.0), safe, Area::Ci, &OPEN_DYSLEXIC);
    assert_eq!(pwa.canvas, Rect::new(0.0, 62.0, 402.0, 778.0));
    assert_no_overflow(&pwa, "402x874 safe");

    let side = SafeArea {
        left: 62.0,
        right: 62.0,
        bottom: 21.0,
        ..SafeArea::ZERO
    };
    let land = solve(Viewport::new(874.0, 402.0), side, Area::Cd, &OPEN_DYSLEXIC);
    assert_eq!(land.breakpoint, Breakpoint::CompactLandscape);
    assert_no_overflow(&land, "874x402 safe");
}

#[test]
fn every_area_uses_a_single_full_canvas_panel() {
    for area in AREAS {
        for (w, h, _) in TARGETS {
            let f = solve(Viewport::new(w, h), SafeArea::ZERO, area, &HURMIT);
            assert_eq!(f.pages, 1);
            assert_eq!(f.panels.len(), 1);
            assert_eq!(f.panels[0].kind, PanelKind::for_area(area));
        }
    }
}

#[test]
fn classification_boundaries() {
    assert_eq!(classify(1280.0, 760.0), Breakpoint::Wide);
    assert_eq!(classify(1279.0, 760.0), Breakpoint::Medium);
    assert_eq!(classify(1280.0, 759.0), Breakpoint::Medium);
    assert_eq!(classify(900.0, 700.0), Breakpoint::Medium);
    assert_eq!(classify(874.0, 479.0), Breakpoint::CompactLandscape);
    assert_eq!(classify(519.0, 2000.0), Breakpoint::Phone);
    assert_eq!(classify(520.0, 480.0), Breakpoint::Medium);
}

#[test]
fn sweep_never_overflows() {
    // Every size between a small phone and a large desktop, both fonts.
    let w_step = if cfg!(miri) { 447.0 } else { 37.0 };
    let h_step = if cfg!(miri) { 323.0 } else { 23.0 };
    let mut w = 320.0;
    while w <= 2560.0 {
        let mut h = 300.0;
        while h <= 1600.0 {
            for font in &CRT_FONTS {
                let f = solve(Viewport::new(w, h), SafeArea::ZERO, Area::Ci, font);
                assert_no_overflow(&f, &format!("{w}x{h} {}", font.family));
            }
            h += h_step;
        }
        w += w_step;
    }
}

#[test]
fn solver_is_deterministic() {
    for (w, h, _) in TARGETS {
        let a = solve(Viewport::new(w, h), SafeArea::ZERO, Area::Ci, &HURMIT);
        let b = solve(Viewport::new(w, h), SafeArea::ZERO, Area::Ci, &HURMIT);
        assert_eq!(a, b);
    }
}

#[test]
fn union_of_rects() {
    assert_eq!(union(&[]), Rect::default());
    let u = union(&[
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Rect::new(5.0, -5.0, 10.0, 10.0),
    ]);
    assert_eq!(u, Rect::new(0.0, -5.0, 15.0, 15.0));
}

#[test]
fn visual_viewport_mapping_and_routes() {
    let v = Viewport::from_visual_viewport(201.0, 360.0, 2.0);
    assert_eq!(v, Viewport::new(402.0, 720.0));
    assert_eq!(
        Viewport::from_visual_viewport(402.0, 720.0, f64::NAN),
        Viewport::new(402.0, 720.0)
    );
    assert_eq!(
        Viewport::from_visual_viewport(402.0, 720.0, 0.0),
        Viewport::new(402.0, 720.0)
    );
    let route = mu_console_protocol::Route {
        area: Area::Fleet,
        subview: mu_console_protocol::SubView::Fleet(
            mu_console_protocol::FleetSubView::WorkerNodes,
        ),
        repo: None,
        submodule: None,
        branch: None,
        job: None,
        trace_id: None,
        filter: None,
    };
    let f = solve_route(
        Viewport::new(1512.0, 860.0),
        SafeArea::ZERO,
        &route,
        &HURMIT,
    );
    assert_eq!(f.panels[0].kind, PanelKind::Fleet);
}
