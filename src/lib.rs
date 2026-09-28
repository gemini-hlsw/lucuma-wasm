//! Handle-based polygon kernel for lucuma-core's Scala.js `ShapeExpression` interpreter.
//!
//! Geometries live in a wasm-side arena and are addressed by `u32` handles. Coordinates are
//! plain `f64` in whatever unit the caller uses (lucuma-core uses microarcseconds with x = -p,
//! y = q). Constructors mirror JTS `GeometricShapeFactory` point placement so results match
//! lucuma-jts closely; see the tests for the reference numbers.
use geo::{
    Area, BooleanOps, BoundingRect, Contains, Coord, Intersects, LineString, MapCoords,
    MultiPolygon, Point, Polygon,
};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

thread_local! {
    static ARENA: RefCell<Vec<Option<MultiPolygon<f64>>>> = const { RefCell::new(Vec::new()) };
    static FREE: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

fn put(g: MultiPolygon<f64>) -> u32 {
    if let Some(h) = FREE.with(|f| f.borrow_mut().pop()) {
        ARENA.with(|a| a.borrow_mut()[h as usize] = Some(g));
        return h;
    }
    ARENA.with(|a| {
        let mut a = a.borrow_mut();
        a.push(Some(g));
        (a.len() - 1) as u32
    })
}

fn with<R>(h: u32, f: impl FnOnce(&MultiPolygon<f64>) -> R) -> R {
    ARENA.with(|a| {
        let a = a.borrow();
        let g = a
            .get(h as usize)
            .and_then(|g| g.as_ref())
            .unwrap_or_else(|| panic!("lucuma-wasm: use of freed or unknown handle {h}"));
        f(g)
    })
}

fn ring(coords: &[f64]) -> MultiPolygon<f64> {
    let pts: Vec<Coord<f64>> = coords
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| Coord { x: c[0], y: c[1] })
        .collect();
    MultiPolygon::new(vec![Polygon::new(LineString::new(pts), vec![])])
}

/// Semver of this kernel, for the facade's compatibility check.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[wasm_bindgen]
pub fn empty_new() -> u32 {
    put(MultiPolygon::new(vec![]))
}

/// Flat closed ring `x0,y0,x1,y1,...`; the caller closes the ring.
#[wasm_bindgen]
pub fn poly_new(coords: &[f64]) -> u32 {
    put(ring(coords))
}

#[wasm_bindgen]
pub fn rect_new(x0: f64, y0: f64, x1: f64, y1: f64) -> u32 {
    let (xa, xb) = (x0.min(x1), x0.max(x1));
    let (ya, yb) = (y0.min(y1), y0.max(y1));
    put(ring(&[xa, ya, xb, ya, xb, yb, xa, yb, xa, ya]))
}

/// JTS `GeometricShapeFactory.createEllipse` from a bounding box: `npts` points starting at
/// angle 0, closed on the first point.
#[wasm_bindgen]
pub fn ellipse_new(x0: f64, y0: f64, x1: f64, y1: f64, npts: u32) -> u32 {
    let rx = (x1 - x0).abs() / 2.0;
    let ry = (y1 - y0).abs() / 2.0;
    let cx = (x0 + x1) / 2.0;
    let cy = (y0 + y1) / 2.0;
    let n = npts.max(3) as usize;
    let mut v = Vec::with_capacity((n + 1) * 2);
    for i in 0..n {
        let ang = i as f64 * (2.0 * std::f64::consts::PI / n as f64);
        v.push(cx + rx * ang.cos());
        v.push(cy + ry * ang.sin());
    }
    v.push(v[0]);
    v.push(v[1]);
    put(ring(&v))
}

/// JTS `GeometricShapeFactory.createArcPolygon`: centre, `npts` points along the arc from
/// `start` over `extent` radians (full circle when extent is out of range), back to centre.
#[wasm_bindgen]
pub fn arc_new(x0: f64, y0: f64, x1: f64, y1: f64, start: f64, extent: f64, npts: u32) -> u32 {
    let rx = (x1 - x0).abs() / 2.0;
    let ry = (y1 - y0).abs() / 2.0;
    let cx = (x0 + x1) / 2.0;
    let cy = (y0 + y1) / 2.0;
    let two_pi = 2.0 * std::f64::consts::PI;
    let size = if extent <= 0.0 || extent > two_pi {
        two_pi
    } else {
        extent
    };
    let n = npts.max(2) as usize;
    let inc = size / (n - 1) as f64;
    let mut v = Vec::with_capacity((n + 2) * 2);
    v.push(cx);
    v.push(cy);
    for i in 0..n {
        let ang = start + inc * i as f64;
        v.push(cx + rx * ang.cos());
        v.push(cy + ry * ang.sin());
    }
    v.push(cx);
    v.push(cy);
    put(ring(&v))
}

/// Boolean overlay. `kind`: 0 intersection, 1 union, 2 difference.
#[wasm_bindgen]
pub fn op(kind: u32, a: u32, b: u32) -> u32 {
    let r = with(a, |ga| {
        with(b, |gb| match kind {
            0 => ga.intersection(gb),
            1 => ga.union(gb),
            _ => ga.difference(gb),
        })
    });
    put(r)
}

/// Affine transform: `x' = m00*x + m01*y + m02`, `y' = m10*x + m11*y + m12`.
#[wasm_bindgen]
pub fn affine(h: u32, m00: f64, m01: f64, m02: f64, m10: f64, m11: f64, m12: f64) -> u32 {
    let r = with(h, |g| {
        g.map_coords(|Coord { x, y }| Coord {
            x: m00 * x + m01 * y + m02,
            y: m10 * x + m11 * y + m12,
        })
    });
    put(r)
}

#[wasm_bindgen]
pub fn area(h: u32) -> f64 {
    with(h, |g| g.unsigned_area())
}

/// `[minx, miny, maxx, maxy]`, all NaN when empty.
#[wasm_bindgen]
pub fn bbox(h: u32) -> Vec<f64> {
    with(h, |g| match g.bounding_rect() {
        Some(r) => vec![r.min().x, r.min().y, r.max().x, r.max().y],
        None => vec![f64::NAN; 4],
    })
}

/// Flat `x,y` coordinates of every exterior ring, concatenated. Enough for a radius or a plot.
#[wasm_bindgen]
pub fn coords(h: u32) -> Vec<f64> {
    with(h, |g| {
        g.iter()
            .flat_map(|p| p.exterior().coords().flat_map(|c| [c.x, c.y]))
            .collect()
    })
}

/// Self-describing ring dump for plotting: `[nPolygons, (nRings, (nPoints, x, y, ...)*)*]`.
/// The first ring of each polygon is its exterior, the rest are holes. Rings are closed.
#[wasm_bindgen]
pub fn rings(h: u32) -> Vec<f64> {
    with(h, |g| {
        let mut out = vec![g.0.len() as f64];
        for p in g.iter() {
            out.push(1.0 + p.interiors().len() as f64);
            for r in std::iter::once(p.exterior()).chain(p.interiors()) {
                out.push(r.0.len() as f64);
                out.extend(r.coords().flat_map(|c| [c.x, c.y]));
            }
        }
        out
    })
}

#[wasm_bindgen]
pub fn contains_point(h: u32, x: f64, y: f64) -> bool {
    with(h, |g| g.contains(&Point::new(x, y)))
}

#[wasm_bindgen]
pub fn intersects(a: u32, b: u32) -> bool {
    with(a, |ga| with(b, |gb| ga.intersects(gb)))
}

#[wasm_bindgen]
pub fn free(h: u32) {
    let was_live = ARENA.with(|a| {
        let mut a = a.borrow_mut();
        match a.get_mut(h as usize) {
            Some(slot) if slot.is_some() => {
                *slot = None;
                true
            }
            _ => false,
        }
    });
    if was_live {
        FREE.with(|f| f.borrow_mut().push(h));
    }
}

/// Number of live handles; a leak detector for the facade's tests.
#[wasm_bindgen]
pub fn live() -> u32 {
    ARENA.with(|a| a.borrow().iter().filter(|g| g.is_some()).count() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn close(a: f64, b: f64, rel: f64) -> bool {
        (a - b).abs() <= rel * a.abs().max(b.abs()).max(1.0)
    }

    #[test]
    fn version_is_cargo_version() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn rectangle_area_and_bbox() {
        let h = rect_new(0.0, 0.0, 4.0, 3.0);
        assert_eq!(area(h), 12.0);
        assert_eq!(bbox(h), vec![0.0, 0.0, 4.0, 3.0]);
        assert!(contains_point(h, 1.0, 1.0));
        assert!(!contains_point(h, 5.0, 1.0));
        free(h);
    }

    #[test]
    fn rings_describe_holes_and_parts() {
        // A square with a square hole: one polygon, two rings, five closed points each.
        let outer = rect_new(0.0, 0.0, 10.0, 10.0);
        let inner = rect_new(4.0, 4.0, 6.0, 6.0);
        let ring_h = op(2, outer, inner);
        let r = rings(ring_h);
        assert_eq!(r[0], 1.0);
        assert_eq!(r[1], 2.0);
        assert_eq!(r[2], 5.0);
        assert_eq!(r[2 + 1 + 10], 5.0);
        assert_eq!(r.len(), 1 + 1 + (1 + 10) * 2);
        // Two disjoint squares: two polygons of one ring each.
        let far = rect_new(20.0, 20.0, 30.0, 30.0);
        let both = op(1, outer, far);
        assert_eq!(rings(both)[0], 2.0);
        for h in [outer, inner, ring_h, far, both] {
            free(h);
        }
    }

    #[test]
    fn ellipse_matches_jts_100_gon() {
        // JTS createEllipse with nPts = 100 on a unit circle: area = 50 * sin(2π/100)
        let h = ellipse_new(-1.0, -1.0, 1.0, 1.0, 100);
        let expected = 50.0 * (2.0 * PI / 100.0).sin();
        assert!(
            close(area(h), expected, 1e-12),
            "{} vs {}",
            area(h),
            expected
        );
        assert_eq!(coords(h).len(), 202);
        free(h);
    }

    #[test]
    fn arc_polygon_quarter_circle() {
        // Quarter circle sector of radius 1, 100 points: (n-1)/2 * sin(π/2/(n-1))
        let h = arc_new(-1.0, -1.0, 1.0, 1.0, 0.0, PI / 2.0, 100);
        let expected = 99.0 / 2.0 * ((PI / 2.0) / 99.0).sin();
        assert!(
            close(area(h), expected, 1e-12),
            "{} vs {}",
            area(h),
            expected
        );
        assert_eq!(coords(h).len(), 204);
        free(h);
    }

    #[test]
    fn overlay_ops() {
        let a = rect_new(0.0, 0.0, 2.0, 2.0);
        let b = rect_new(1.0, 1.0, 3.0, 3.0);
        let i = op(0, a, b);
        let u = op(1, a, b);
        let d = op(2, a, b);
        assert!(close(area(i), 1.0, 1e-12));
        assert!(close(area(u), 7.0, 1e-12));
        assert!(close(area(d), 3.0, 1e-12));
        assert!(intersects(a, b));
        for h in [a, b, i, u, d] {
            free(h);
        }
        assert_eq!(live(), 0);
    }

    #[test]
    fn affine_rotation_keeps_area_and_moves_bbox() {
        let h = rect_new(0.0, 0.0, 2.0, 1.0);
        let (c, s) = ((PI / 2.0).cos(), (PI / 2.0).sin());
        let r = affine(h, c, -s, 0.0, s, c, 0.0);
        assert!(close(area(r), 2.0, 1e-12));
        let bb = bbox(r);
        assert!(close(bb[0], -1.0, 1e-12) && close(bb[3], 2.0, 1e-12));
        free(h);
        free(r);
    }

    #[test]
    fn empty_shape_behaves() {
        let e = empty_new();
        assert_eq!(area(e), 0.0);
        assert!(bbox(e)[0].is_nan());
        assert!(!contains_point(e, 0.0, 0.0));
        let r = rect_new(0.0, 0.0, 1.0, 1.0);
        let i = op(0, e, r);
        assert_eq!(area(i), 0.0);
        free(e);
        free(r);
        free(i);
    }

    #[test]
    fn free_is_idempotent_and_handles_recycle() {
        let a = rect_new(0.0, 0.0, 1.0, 1.0);
        free(a);
        free(a);
        let b = rect_new(0.0, 0.0, 1.0, 1.0);
        assert_eq!(a, b);
        free(b);
        assert_eq!(live(), 0);
    }
}
