//! Original starting geometries inspired by classic topologies, not copied
//! commercial prescriptions. Every outcome is verified against the user's brief.
use crate::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignBrief {
    pub structure: String,
    pub focal_length_mm: f64,
    pub f_number: f64,
    pub sensor_mm: [f64; 2],
    pub half_field_deg: f64,
    pub max_length_mm: f64,
    pub max_rms_um: f64,
    pub iterations: usize,
    pub starts: usize,
    #[serde(default)]
    pub require_sensor_coverage: bool,
    /// Bounded catalog pairs; each pair is optimized again from a scaled seed.
    #[serde(default)]
    pub glass_search: bool,
}
#[derive(Serialize)]
pub struct DesignOutcome {
    pub candidate: Option<Candidate>,
    pub candidates: Vec<Candidate>,
    pub attempts: Vec<String>,
    pub brief: DesignBrief,
    pub note: String,
}
pub fn seed(kind: &str) -> Result<Project, String> {
    let mut p = examples().remove(3);
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../../../src/data/glass-catalog.json")).unwrap();
    for (index, name) in [(1, "SCHOTT N-BK7"), (2, "SCHOTT N-F2")] {
        let item = catalog["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == name)
            .ok_or("Missing seed glass")?;
        p.materials[index] = serde_json::from_value(item.clone()).map_err(|e| e.to_string())?;
    }
    // radius, following thickness, following material. The explicit plane is
    // an air stop. Cemented boundaries share one refracting surface.
    let (rows, stop): (Vec<(f64, f64, usize)>, usize) = match kind {
        "double_gauss" => (
            p.surfaces
                .iter()
                .map(|s| {
                    (
                        if s.curvature == 0. {
                            0.
                        } else {
                            1. / s.curvature
                        },
                        s.thickness,
                        s.material,
                    )
                })
                .collect(),
            p.stop,
        ),
        "planar" => (
            vec![
                (70., 4., 1),
                (-200., 2., 0),
                (45., 4., 1),
                (100., 2., 2),
                (35., 4., 0),
                (0., 4., 0),
                (-35., 2., 2),
                (-100., 4., 1),
                (-45., 2., 0),
                (200., 4., 1),
                (-70., 60., 0),
            ],
            5,
        ),
        "tessar" => (
            vec![
                (40., 4., 1),
                (-70., 4., 0),
                (-35., 2., 2),
                (35., 3., 0),
                (0., 3., 0),
                (60., 2., 2),
                (30., 4., 1),
                (-40., 55., 0),
            ],
            4,
        ),
        "sonnar" => (
            vec![
                (45., 4., 1),
                (-100., 3., 0),
                (50., 4., 1),
                (150., 2., 2),
                (30., 4., 1),
                (80., 4., 0),
                (0., 4., 0),
                (60., 2., 2),
                (30., 4., 1),
                (-45., 55., 0),
            ],
            6,
        ),
        _ => return Err("Unknown classic structure".into()),
    };
    p.surfaces = rows
        .into_iter()
        .enumerate()
        .map(|(i, (r, t, m))| Surface {
            name: format!(
                "S{} · {}",
                i + 1,
                if i == stop {
                    "光阑"
                } else {
                    "经典结构初值"
                }
            ),
            curvature: if r == 0. { 0. } else { 1. / r },
            conic: 0.,
            asphere: [0.; 4],
            thickness: t,
            semi_aperture: if i == stop { 2. } else { 8. },
            material: m,
            mechanical: None,
            coating: vec![],
        })
        .collect();
    p.stop = stop;
    p.fields = vec![0., 0.025, 0.05];
    p.field_spec = None;
    p.name = format!("{} · 原创设计起点", kind);
    p.description =
        "OOS 原创参数；仅借鉴经典结构拓扑，不是 ZEISS 或其他厂商处方；需按目标优化和验证。".into();
    Ok(p)
}
pub fn run(
    b: &DesignBrief,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(&OptimizationProgress),
) -> Result<DesignOutcome, String> {
    if ![
        b.focal_length_mm,
        b.f_number,
        b.sensor_mm[0],
        b.sensor_mm[1],
        b.half_field_deg,
        b.max_length_mm,
        b.max_rms_um,
    ]
    .iter()
    .all(|v| v.is_finite())
        || !(10. ..=1000.).contains(&b.focal_length_mm)
        || !(1. ..=64.).contains(&b.f_number)
        || b.sensor_mm.iter().any(|v| *v <= 0. || *v > 100.)
        || !(0. ..=40.).contains(&b.half_field_deg)
        || b.max_length_mm <= 0.
        || b.max_rms_um <= 0.
        || !(1..=100).contains(&b.iterations)
        || !(1..=3).contains(&b.starts)
    {
        return Err(
            "设计指标无效：焦距 10–1000 mm、F/1–64、半视场 0–40°，1–3 个起点与 1–100 次迭代。"
                .into(),
        );
    }
    let structures: Vec<&str> = if b.structure == "auto" {
        vec!["tessar", "planar", "double_gauss", "sonnar"]
    } else {
        vec![&b.structure]
    };
    let pairs = if b.glass_search {
        vec![
            ("SCHOTT N-BK7", "SCHOTT N-F2"),
            ("SCHOTT N-BAK4", "SCHOTT N-SF5"),
            ("SCHOTT N-SK16", "SCHOTT N-SF10"),
        ]
    } else {
        vec![("SCHOTT N-BK7", "SCHOTT N-F2")]
    };
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../../../src/data/glass-catalog.json"))
            .map_err(|e| e.to_string())?;
    let mut candidates = vec![];
    let mut attempts = vec![];
    let mut winner: Option<Candidate> = None;
    for structure in structures {
        for &(crown, flint) in &pairs {
            let mut original = seed(structure)?;
            for (index, name) in [(1, crown), (2, flint)] {
                let item = catalog["entries"]
                    .as_array()
                    .ok_or("Invalid catalog")?
                    .iter()
                    .find(|v| v["name"] == name)
                    .ok_or("Missing search glass")?;
                original.materials[index] =
                    serde_json::from_value(item.clone()).map_err(|e| e.to_string())?;
            }
            for start in 0..b.starts {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                let result = (|| {
                    let mut p = original.clone();
                    // Distinct deterministic bending starts, followed by exact first-order scaling.
                    for s in &mut p.surfaces {
                        if s.curvature != 0. {
                            s.curvature *= 1. + (start as f64) * 0.06 * s.curvature.signum();
                        }
                    }
                    let efl = paraxial(&p, 587.5618)?
                        .efl
                        .filter(|f| *f > 0.)
                        .ok_or("Seed has no positive focal length")?;
                    let scale = b.focal_length_mm / efl;
                    for s in &mut p.surfaces {
                        s.curvature /= scale;
                        s.thickness *= scale;
                        s.semi_aperture *= scale;
                    }
                    let clear = b.focal_length_mm / (2. * b.f_number) * 1.8
                        + b.focal_length_mm
                            * if b.require_sensor_coverage {
                                b.sensor_mm[0].hypot(b.sensor_mm[1]) / (2. * b.focal_length_mm)
                            } else {
                                b.half_field_deg.to_radians().tan()
                            }
                            * 0.2;
                    for (i, s) in p.surfaces.iter_mut().enumerate() {
                        if i != p.stop {
                            s.semi_aperture = s.semi_aperture.max(clear);
                        }
                    }
                    for i in 0..p.surfaces.len() - 1 {
                        let radius = p.surfaces[i]
                            .semi_aperture
                            .min(p.surfaces[i + 1].semi_aperture);
                        let mut required = 0.01;
                        for j in 0..=64 {
                            let r = radius * j as f64 / 64.;
                            let a = p.surfaces[i]
                                .sag(r)
                                .ok_or("Requested aperture exceeds seed surface domain")?
                                .0;
                            let b = p.surfaces[i + 1]
                                .sag(r)
                                .ok_or("Requested aperture exceeds seed surface domain")?
                                .0;
                            required = f64::max(required, a - b + 0.5 * scale);
                        }
                        p.surfaces[i].thickness = p.surfaces[i].thickness.max(required);
                    }
                    p.sensor = b.sensor_mm;
                    p.fields = vec![
                        0.,
                        b.half_field_deg.to_radians() * 0.7,
                        b.half_field_deg.to_radians(),
                    ];
                    let last = p.surfaces.len() - 1;
                    p.surfaces[last].thickness = paraxial(&p, 587.5618)?
                        .bfl
                        .filter(|v| *v > 0.)
                        .ok_or("Seed has virtual focus")?;
                    let mut final_candidate = None;
                    // Continuation improves the seed before applying the complete aperture/field.
                    for fraction in [0.25, 0.5, 1.] {
                        if cancel.load(Ordering::Relaxed) {
                            return Err("Cancelled".into());
                        }
                        p.field_spec = if b.require_sensor_coverage {
                            Some(fields::FieldSpec::ImageHeights {
                                heights_mm: vec![
                                    0.,
                                    b.sensor_mm[0].min(b.sensor_mm[1]) / 2. * fraction,
                                    b.sensor_mm[0].max(b.sensor_mm[1]) / 2. * fraction,
                                    b.sensor_mm[0].hypot(b.sensor_mm[1]) / 2. * fraction,
                                ],
                            })
                        } else {
                            None
                        };
                        p.fields = vec![
                            0.,
                            b.half_field_deg.to_radians() * 0.7 * fraction,
                            b.half_field_deg.to_radians() * fraction,
                        ];
                        if b.require_sensor_coverage {
                            p.schema_version = 3;
                            p.fields = vec![0.; 4];
                        }
                        let target_f = b.f_number / fraction;
                        let para = paraxial(&p, 587.5618)?;
                        let actual = para.f_number.ok_or("Cannot locate entrance pupil")?;
                        p.surfaces[p.stop].semi_aperture *= actual / target_f;
                        let diagnostics = p.validate_optical();
                        if !diagnostics.is_empty() {
                            return Err(format!(
                                "孔径/视场阶段 {:.0}%：{}",
                                fraction * 100.,
                                diagnostics
                                    .iter()
                                    .map(|d| format!(
                                        "{}{}",
                                        d.surface
                                            .map(|i| format!("S{}：", i + 1))
                                            .unwrap_or_default(),
                                        d.message
                                    ))
                                    .collect::<Vec<_>>()
                                    .join("；")
                            ));
                        }
                        p = autofocus(&p, cancel)?.project;
                        let variables = p
                            .surfaces
                            .iter()
                            .enumerate()
                            .flat_map(|(i, s)| {
                                let mut v = vec![VariableSetting {
                                    variable: Variable::Thickness { surface: i },
                                    min: Some(if s.material == 0 { 0.01 } else { 0.5 * scale }),
                                    max: Some((s.thickness * 2.).max(scale)),
                                    locked: false,
                                }];
                                if i != p.stop {
                                    v.push(VariableSetting {
                                        variable: Variable::Curvature { surface: i },
                                        min: Some(s.curvature - 0.02 / scale),
                                        max: Some(s.curvature + 0.02 / scale),
                                        locked: false,
                                    });
                                }
                                v
                            })
                            .collect();
                        let constraints = Constraints {
                            target_efl_mm: b.focal_length_mm,
                            efl_tolerance_mm: b.focal_length_mm * 0.01,
                            max_length_mm: b.max_length_mm,
                            min_center_mm: 0.5 * scale,
                            min_edge_mm: 0.1 * scale,
                            min_air_mm: 0.,
                            design: Some(DesignTargets {
                                f_number: target_f,
                                f_tolerance: target_f * 0.03,
                                min_bfl_mm: 0.01,
                                max_front_diameter_mm: 1000.,
                            }),
                        };
                        let options = OptimizationOptions {
                            quality: Some(quality::QualityGuards {
                                max_worst_rms_um: Some(b.max_rms_um),
                                ..Default::default()
                            }),
                            staged: false,
                            refocus_each_stage: false,
                            variables,
                            preset: AnalysisPreset::Balanced,
                            max_iterations: b.iterations,
                        };
                        let c = optimize_configured(&p, &options, &constraints, cancel, progress)?;
                        p = c.project.clone();
                        final_candidate = Some(c);
                    }
                    Ok::<_, String>(final_candidate.unwrap())
                })();
                match result {
                    Ok(mut c) => {
                        c.project.name =
                            format!("{structure} · {} mm · {crown} / {flint}", b.focal_length_mm);
                        if b.require_sensor_coverage {
                            let analysis = analyze(&c.project, 33, cancel)?;
                            for point in analysis
                                .coverage
                                .as_ref()
                                .ok_or("Missing coverage")?
                                .checkpoints
                                .iter()
                            {
                                c.checks.push(ConstraintCheck {
                                    name: format!(
                                        "传感器覆盖 · {} ({:.3} mm)",
                                        point.label, point.height_mm
                                    ),
                                    surface: None,
                                    actual: Some(if point.status == "sampled_clear" {
                                        1.
                                    } else {
                                        0.
                                    }),
                                    limit: 1.,
                                    relation: "min".into(),
                                    passed: point.status == "sampled_clear",
                                    reason: Some(format!(
                                        "33×33 共用平面像面；{}；MTF 独立验收",
                                        point.status
                                    )),
                                });
                            }
                            c.accepted = c.checks.iter().all(|v| v.passed);
                        }
                        candidates.push(c.clone());
                        attempts.push(format!(
                            "{structure} · {crown}/{flint} · 起点 {}：{}；RMS {:.3} µm",
                            start + 1,
                            if c.accepted {
                                "目标检查通过"
                            } else {
                                "目标尚未达成"
                            },
                            c.after_rms_um
                        ));
                        if winner.as_ref().is_none_or(|w| {
                            (c.accepted && !w.accepted)
                                || (c.accepted == w.accepted && c.after_rms_um < w.after_rms_um)
                        }) {
                            winner = Some(c);
                        }
                    }
                    Err(e) => {
                        if cancel.load(Ordering::Relaxed) {
                            return Err("Cancelled".into());
                        }
                        let friendly=serde_json::from_str::<serde_json::Value>(&e).ok()
                    .filter(|v|v["code"]=="ray_failures").map(|v|{
                        let failed=v["failures"].as_array().map(|rows|rows.iter()
                            .filter_map(|r|r["failed"].as_u64()).sum::<u64>()).unwrap_or(0);
                        format!("当前阶段有 {failed} 条失败光线。请缩小孔径/视场、放宽长度或改用其他结构；失败样本未进入零误差评分。")
                    }).unwrap_or(e);
                        attempts.push(format!(
                            "{structure} · {crown}/{flint} · 起点 {}：{}",
                            start + 1,
                            friendly
                        ));
                    }
                }
            }
        }
    }
    candidates.sort_by(|a, b| {
        b.accepted
            .cmp(&a.accepted)
            .then(a.after_rms_um.total_cmp(&b.after_rms_um))
    });
    Ok(DesignOutcome{candidate:winner,candidates,attempts,brief:b.clone(),
        note:"经典结构约束设计；仅在所选视场、F/d/C 波长和 33×33 几何采样上验证。覆盖以检查表中实际像高的通过项为准；MTF、制造公差和环境适应性需另行验证；不保证任意指标可解。".into()})
}
