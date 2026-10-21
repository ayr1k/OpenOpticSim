//! Bounded feasibility restoration. Never changes a locked or unselected variable.
use crate::optimize::{set, value};
use crate::*;

fn assign(p: &mut Project, variables: &[VariableSetting], v: Variable, x: f64) {
    if !x.is_finite() {
        return;
    }
    if let Some(s) = variables.iter().find(|s| !s.locked && s.variable == v) {
        set(
            p,
            &v,
            x.clamp(
                s.min.unwrap_or(f64::NEG_INFINITY),
                s.max.unwrap_or(f64::INFINITY),
            ),
        );
    }
}

/// Cheap first-order correction plus geometric clearance restoration. This is
/// not acceptance: dense trace and continuous clearance certification remain mandatory.
pub fn restore(p: &mut Project, variables: &[VariableSetting], c: &Constraints) {
    for _ in 0..4 {
        let geometry = geometry::inspect(p);
        for span in geometry.spans {
            let minimum = if span.glass {
                c.min_edge_mm
            } else {
                c.min_air_mm
            };
            let center = if span.glass {
                c.min_center_mm
            } else {
                c.min_air_mm
            };
            let Some(measured) = span.sampled_min_mm else {
                continue;
            };
            let margin = 1e-4_f64.max(minimum * 1e-3);
            let delta = (minimum + margin - measured).max(center + margin - span.center_mm);
            if delta <= 0. {
                continue;
            }
            let mut remaining = delta;
            for i in span.front..span.back {
                let old = p.surfaces[i].thickness;
                assign(
                    p,
                    variables,
                    Variable::Thickness { surface: i },
                    old + remaining,
                );
                remaining -= p.surfaces[i].thickness - old;
                if remaining <= 1e-12 {
                    break;
                }
            }
        }
        // Minimum-norm correction in scaled curvature coordinates. Safeguarded
        // by a line search; image distance is never used to fake focal length.
        for _ in 0..4 {
            let Some(f) = paraxial(p, 587.5618)
                .ok()
                .and_then(|a| a.efl)
                .filter(|f| f.is_finite())
            else {
                break;
            };
            let error = f - c.target_efl_mm;
            if error.abs() <= c.efl_tolerance_mm * 0.1 {
                break;
            }
            let mut columns = vec![];
            for s in variables
                .iter()
                .filter(|s| !s.locked && matches!(s.variable, Variable::Curvature { .. }))
            {
                let x = value(p, &s.variable).unwrap();
                let scale = x.abs().max(1e-3);
                let h = scale * 1e-4;
                let mut q = p.clone();
                assign(&mut q, variables, s.variable.clone(), x + h);
                let actual_h = value(&q, &s.variable).unwrap() - x;
                if actual_h.abs() < 1e-15 {
                    continue;
                }
                if let Some(ff) = paraxial(&q, 587.5618).ok().and_then(|a| a.efl) {
                    let derivative = (ff - f) / actual_h * scale;
                    if derivative.is_finite() {
                        columns.push((s.variable.clone(), x, scale, derivative));
                    }
                }
            }
            let norm = columns.iter().map(|v| v.3 * v.3).sum::<f64>();
            if norm < 1e-20 {
                break;
            }
            let mut improved = false;
            for fraction in [1., 0.5, 0.25, 0.125] {
                let mut q = p.clone();
                for (v, x, scale, d) in &columns {
                    assign(
                        &mut q,
                        variables,
                        v.clone(),
                        x + fraction * (-error * d / norm).clamp(-0.2, 0.2) * scale,
                    );
                }
                if paraxial(&q, 587.5618)
                    .ok()
                    .and_then(|a| a.efl)
                    .is_some_and(|ff| (ff - c.target_efl_mm).abs() < error.abs())
                {
                    *p = q;
                    improved = true;
                    break;
                }
            }
            if !improved {
                break;
            }
        }
        if let Some(target) = &c.design {
            if let Some(actual) = paraxial(p, 587.5618).ok().and_then(|a| a.f_number) {
                let radius = p.surfaces[p.stop].semi_aperture * actual / target.f_number;
                assign(p, variables, Variable::Aperture { surface: p.stop }, radius);
            }
        }
    }
}

/// Feasibility ranks before image quality. Missing values never score zero.
pub fn violation(p: &Project, c: &Constraints) -> f64 {
    constraint_checks(p, c)
        .iter()
        .filter(|v| !v.passed)
        .map(|v| {
            v.actual
                .map(|a| ((a - v.limit).abs() / v.limit.abs().max(0.01)).max(1e-6))
                .unwrap_or(1e6)
        })
        .sum()
}
