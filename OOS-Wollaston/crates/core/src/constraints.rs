use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Constraints {
    pub target_efl_mm: f64,
    pub efl_tolerance_mm: f64,
    /// First vertex to sensor, including the last image distance.
    pub max_length_mm: f64,
    pub min_center_mm: f64,
    /// Minimum throughout each glass body, including the mechanical edge.
    pub min_edge_mm: f64,
    /// Minimum throughout shared air profiles; includes independent stops.
    pub min_air_mm: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<DesignTargets>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DesignTargets {
    pub f_number: f64,
    pub f_tolerance: f64,
    pub min_bfl_mm: f64,
    pub max_front_diameter_mm: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConstraintCheck {
    pub name: String,
    pub surface: Option<usize>,
    pub actual: Option<f64>,
    pub limit: f64,
    pub relation: String,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
impl Constraints {
    pub fn validate(&self) -> Result<(), String> {
        if ![
            self.target_efl_mm,
            self.efl_tolerance_mm,
            self.max_length_mm,
            self.min_center_mm,
            self.min_edge_mm,
            self.min_air_mm,
        ]
        .iter()
        .all(|v| v.is_finite())
            || self.target_efl_mm <= 0.
            || self.efl_tolerance_mm <= 0.
            || self.max_length_mm <= 0.
            || self.min_center_mm < 0.
            || self.min_edge_mm < 0.
            || self.min_air_mm < 0.
        {
            return Err("Constraints require positive EFL, EFL tolerance and total length; nonnegative thickness and air gap".into());
        }
        if self.design.as_ref().is_some_and(|g| {
            ![
                g.f_number,
                g.f_tolerance,
                g.min_bfl_mm,
                g.max_front_diameter_mm,
            ]
            .iter()
            .all(|v| v.is_finite())
                || g.f_number < 0.5
                || g.f_number > 128.
                || g.f_tolerance <= 0.
                || g.min_bfl_mm < 0.
                || g.max_front_diameter_mm <= 0.
        }) {
            return Err("设计目标：F 数 0.5–128，容差与前组直径须为正，最小后焦距非负".into());
        }
        Ok(())
    }
}
pub fn constraint_checks(p: &Project, c: &Constraints) -> Vec<ConstraintCheck> {
    let f = paraxial(p, 587.5618).ok().and_then(|a| a.efl);
    let error = f.map(|v| (v - c.target_efl_mm).abs());
    let mut checks = vec![
        ConstraintCheck {
            name: "efl_error_mm".into(),
            surface: None,
            actual: error,
            limit: c.efl_tolerance_mm,
            relation: "max".into(),
            passed: error.is_some_and(|v| v <= c.efl_tolerance_mm + 1e-9),
            reason: None,
        },
        ConstraintCheck {
            name: "total_length_mm".into(),
            surface: None,
            actual: Some(p.image_z()),
            limit: c.max_length_mm,
            relation: "max".into(),
            passed: p.image_z() <= c.max_length_mm + 1e-9,
            reason: None,
        },
    ];
    if let Some(g) = &c.design {
        let para = paraxial(p, 587.5618).ok();
        let fn_error = para
            .as_ref()
            .and_then(|a| a.f_number)
            .map(|v| (v - g.f_number).abs());
        let bfl = para.and_then(|a| a.bfl);
        for (name, actual, limit, relation) in [
            ("f_number_error", fn_error, g.f_tolerance, "max"),
            ("bfl_mm", bfl, g.min_bfl_mm, "min"),
            (
                "front_diameter_mm",
                p.surfaces.first().map(|s| s.semi_aperture * 2.),
                g.max_front_diameter_mm,
                "max",
            ),
        ] {
            checks.push(ConstraintCheck {
                name: name.into(),
                surface: None,
                actual,
                limit,
                relation: relation.into(),
                passed: actual.is_some_and(|v| {
                    if relation == "min" {
                        v + 1e-9 >= limit
                    } else {
                        v <= limit + 1e-9
                    }
                }),
                reason: None,
            });
        }
    }
    for span in geometry::inspect(p).spans {
        if span.glass {
            checks.push(ConstraintCheck {
                name: "center_mm".into(),
                surface: Some(span.front),
                actual: Some(span.center_mm),
                limit: c.min_center_mm,
                relation: "min".into(),
                passed: span.center_mm + 1e-9 >= c.min_center_mm,
                reason: None,
            });
        }
        let limit = if span.glass {
            c.min_edge_mm
        } else {
            c.min_air_mm
        };
        checks.push(ConstraintCheck {
            name: if span.glass {
                "body_min_mm"
            } else {
                "air_min_mm"
            }
            .into(),
            surface: Some(span.front),
            actual: span.sampled_min_mm,
            limit,
            relation: "min".into(),
            passed: span.certified && geometry::clearance_at_least(p, &span, limit),
            reason: Some(format!(
                "S{}–S{}：显示值为离散采样估计；判定使用完整半径上的连续区间检查。{}",
                span.front + 1,
                span.back + 1,
                if !span.certified {
                    "几何分离无法确认，请检查几何面板。"
                } else if !geometry::clearance_at_least(p, &span, limit) {
                    "无法确认处处达到最小间距；即使显示值四舍五入后达标，也不能采纳。"
                } else {
                    "最小间距已确认。"
                }
            )),
        });
    }
    // The sensor is planar and only a positive axial image distance is modeled.
    if let Some(last) = p.surfaces.last() {
        checks.push(ConstraintCheck {
            name: "image_gap_mm".into(),
            surface: Some(p.surfaces.len() - 1),
            actual: Some(last.thickness),
            limit: c.min_air_mm,
            relation: "min".into(),
            passed: last.thickness + 1e-9 >= c.min_air_mm,
            reason: None,
        });
    }
    checks
}

pub(crate) fn penalties(p: &Project, c: &Constraints) -> Vec<f64> {
    // Dimensionless residuals. Use sampled thickness for derivatives, continuous
    // certification for final acceptance; do not mislabel sampling as a proof.
    let mut out = vec![((p.image_z() - c.max_length_mm) / c.max_length_mm.max(1.)).max(0.) * 100.];
    if let Some(g) = &c.design {
        let para = paraxial(p, 587.5618).ok();
        out.push(
            para.as_ref()
                .and_then(|a| a.f_number)
                .map_or(1e6, |f| (f - g.f_number) / g.f_tolerance),
        );
        out.push(para.and_then(|a| a.bfl).map_or(1e6, |b| {
            (g.min_bfl_mm - b).max(0.) / g.min_bfl_mm.max(1.) * 100.
        }));
        out.push(
            (p.surfaces[0].semi_aperture * 2. - g.max_front_diameter_mm).max(0.)
                / g.max_front_diameter_mm
                * 100.,
        );
    }
    for span in geometry::inspect(p).spans {
        if span.glass {
            out.push((c.min_center_mm - span.center_mm).max(0.) / c.min_center_mm.max(0.1) * 100.);
        }
        let min = if span.glass {
            c.min_edge_mm
        } else {
            c.min_air_mm
        };
        out.push((min - span.sampled_min_mm.unwrap_or(-1e6)).max(0.) / min.max(0.1) * 100.);
    }
    out.push(
        (c.min_air_mm - p.surfaces.last().unwrap().thickness).max(0.) / c.min_air_mm.max(0.1)
            * 100.,
    );
    out
}
