//! Mechanical profiles are non-imaging surfaces. Certification uses derivative
//! intervals, split at annulus boundaries; a sampling grid alone is not a pass.
use crate::{Diagnostic, EdgeShape, Project, Surface};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Span {
    pub front: usize,
    pub back: usize,
    pub glass: bool,
    pub center_mm: f64,
    pub edge_mm: Option<f64>,
    pub sampled_min_mm: Option<f64>,
    pub radius_mm: f64,
    pub certified: bool,
}
#[derive(Debug, Serialize)]
pub struct Geometry {
    pub spans: Vec<Span>,
    pub diagnostics: Vec<Diagnostic>,
}

// Every term is monotonic on r >= 0. Summing their intervals gives a
// conservative enclosure even when high-order terms cancel one another.
fn slope(s: &Surface, lo: f64, hi: f64) -> Option<(f64, f64)> {
    if matches!(
        s.mechanical.as_ref().map(|m| &m.shape),
        Some(EdgeShape::Flat)
    ) && lo >= s.semi_aperture
    {
        return Some((0., 0.));
    }
    let base = |r: f64| {
        let q = 1. - (1. + s.conic) * s.curvature.powi(2) * r * r;
        (q > 1e-14).then(|| s.curvature * r / q.sqrt())
    };
    let (a, b) = (base(lo)?, base(hi)?);
    let (mut lower, mut upper) = (a.min(b), a.max(b));
    for (i, coefficient) in s.asphere.iter().enumerate() {
        let power = 4 + i as i32 * 2;
        let (a, b) = (
            power as f64 * coefficient * lo.powi(power - 1),
            power as f64 * coefficient * hi.powi(power - 1),
        );
        lower += a.min(b);
        upper += a.max(b);
    }
    (lower.is_finite() && upper.is_finite()).then_some((lower, upper))
}

fn check(a: &Surface, b: &Surface, distance: f64, radius: f64) -> (Option<f64>, bool, bool) {
    let gap = |r| Some(distance + b.profile(r)? - a.profile(r)?);
    if a.curvature == b.curvature
        && a.conic == b.conic
        && a.asphere == b.asphere
        && a.semi_aperture == b.semi_aperture
        && a.mechanical == b.mechanical
    {
        return (Some(distance), distance >= -1e-9, distance < -1e-9);
    }
    let mut minimum = f64::INFINITY;
    // Display value, plus early rejection. Certification below is independent.
    for i in 0..=256 {
        let Some(g) = gap(radius * i as f64 / 256.) else {
            return (None, false, false);
        };
        minimum = minimum.min(g);
    }
    if minimum < -1e-9 {
        return (Some(minimum), false, true);
    }
    let mut cuts = vec![0., radius];
    for s in [a, b] {
        if s.semi_aperture > 0. && s.semi_aperture < radius {
            cuts.push(s.semi_aperture);
        }
    }
    cuts.sort_by(f64::total_cmp);
    let mut stack: Vec<_> = cuts.windows(2).map(|w| (w[0], w[1])).collect();
    let mut count = 0;
    while let Some((lo, hi)) = stack.pop() {
        count += 1;
        if count > 16384 {
            return (Some(minimum), false, false);
        }
        let mid = (lo + hi) * 0.5;
        let Some(g) = gap(mid) else {
            return (None, false, false);
        };
        minimum = minimum.min(g);
        if g < -1e-9 {
            return (Some(minimum), false, true);
        }
        let (Some(sa), Some(sb)) = (slope(a, lo, hi), slope(b, lo, hi)) else {
            return (Some(minimum), false, false);
        };
        let (lower, upper) = (sb.0 - sa.1, sb.1 - sa.0);
        let bound = if lower >= 0. {
            gap(lo).unwrap_or(f64::NEG_INFINITY)
        } else if upper <= 0. {
            gap(hi).unwrap_or(f64::NEG_INFINITY)
        } else {
            g - lower.abs().max(upper.abs()) * (hi - lo) * 0.5
        };
        if bound >= -1e-9 {
            continue;
        }
        if hi - lo < 1e-12 * radius.max(1.) {
            return (Some(minimum), false, false);
        }
        stack.push((lo, mid));
        stack.push((mid, hi));
    }
    (Some(minimum), true, false)
}

pub fn inspect(p: &Project) -> Geometry {
    let mut result = Geometry {
        spans: vec![],
        diagnostics: vec![],
    };
    let z = p.vertices();
    let mut pairs: Vec<_> = (0..p.surfaces.len().saturating_sub(1))
        .map(|i| (i, i + 1))
        .collect();
    // Check air between lens bodies even when an intervening stop has a much
    // smaller aperture. Otherwise it could hide a mechanical collision.
    let bodies: Vec<_> = (0..p.surfaces.len().saturating_sub(1))
        .filter(|i| p.surfaces[*i].material != 0)
        .collect();
    for pair in bodies.windows(2) {
        if pair[1] > pair[0] + 2 {
            pairs.push((pair[0] + 1, pair[1]));
        }
    }
    for (i, j) in pairs {
        let (a, b) = (&p.surfaces[i], &p.surfaces[j]);
        let radius = a.outer_radius().min(b.outer_radius());
        let distance = z[j] - z[i];
        let (minimum, certified, intersect) =
            if radius.is_finite() && radius > 0. && radius <= 1e4 && distance.is_finite() {
                check(a, b, distance, radius)
            } else {
                (None, false, false)
            };
        let edge = a
            .profile(radius)
            .zip(b.profile(radius))
            .map(|(a, b)| distance + b - a)
            .filter(|x| x.is_finite());
        if !certified {
            result.diagnostics.push(Diagnostic {
                surface: Some(i),
                message: if intersect {
                    format!(
                        "Geometry intersection with surface {} (optical or mechanical profile)",
                        j + 1
                    )
                } else {
                    format!(
                        "Geometry could not certify separation from surface {}",
                        j + 1
                    )
                },
            });
        }
        result.spans.push(Span {
            front: i,
            back: j,
            glass: a.material != 0,
            center_mm: distance,
            edge_mm: edge,
            sampled_min_mm: minimum,
            radius_mm: radius,
            certified,
        });
    }
    result
}

/// Certify a requested minimum over the entire shared optical/mechanical radius.
pub fn clearance_at_least(p: &Project, span: &Span, minimum: f64) -> bool {
    let a = &p.surfaces[span.front];
    let b = &p.surfaces[span.back];
    check(a, b, span.center_mm - minimum, span.radius_mm).1
}
