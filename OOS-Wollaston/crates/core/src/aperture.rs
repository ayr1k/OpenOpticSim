//! Aperture sweeps and physical stop-coordinate maps. Never infer illumination from counts.
use crate::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApertureOptions {
    #[serde(default)]
    pub wave_method: wave::WaveMethod,
    pub f_numbers: Vec<f64>,
    pub refocus: bool,
    pub sampling: usize,
    pub mtf: bool,
}
#[derive(Debug, Serialize)]
pub struct ApertureMtf {
    pub method: wave::WaveMethod,
    pub field: f64,
    /// [S10, T10, S30, T30], lp/mm. Null means this frequency is outside the grid.
    pub values: Option<[Option<f64>; 4]>,
    pub quality: Option<wave::WaveQuality>,
    pub error: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct AperturePoint {
    pub coverage: Option<crate::fields::Coverage>,
    pub fields_rad: Vec<f64>,
    pub requested_f_number: f64,
    pub actual_f_number: Option<f64>,
    pub stop_radius_mm: Option<f64>,
    pub image_distance_mm: f64,
    pub fields_rms_um: Vec<Option<f64>>,
    pub score_um: Option<f64>,
    pub failed: usize,
    pub sampled: usize,
    pub aiming_failed: usize,
    pub blocked: usize,
    pub mtf: Vec<ApertureMtf>,
    pub diagnostic: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ApertureScan {
    pub source: Project,
    pub options: ApertureOptions,
    pub points: Vec<AperturePoint>,
    pub best_index: Option<usize>,
}
pub fn set_stop_radius(p: &mut Project, radius: f64) {
    p.surfaces[p.stop].semi_aperture = radius;
    if let Some(edge) = p.surfaces[p.stop].mechanical.as_mut() {
        edge.semi_diameter = edge.semi_diameter.max(radius);
    }
}
pub fn at_f_number(p: &Project, f: f64) -> Result<Project, String> {
    if !f.is_finite() || !(0.5..=128.).contains(&f) {
        return Err("目标 F 数须在 0.5–128 之间".into());
    }
    let actual = paraxial(p, 587.5618)?
        .f_number
        .filter(|v| v.is_finite() && *v > 0.)
        .ok_or("当前系统无法定义有限入瞳与 F 数")?;
    let radius = p.surfaces[p.stop].semi_aperture * actual / f;
    if !radius.is_finite() || radius <= 0. || radius > 10000. {
        return Err("所需实体光阑超出允许范围".into());
    }
    let mut q = p.clone();
    set_stop_radius(&mut q, radius);
    Ok(q)
}
pub fn aperture_scan(
    p: &Project,
    o: &ApertureOptions,
    cancel: &AtomicBool,
) -> Result<ApertureScan, String> {
    if !p.validate_optical().is_empty() {
        return Err("光圈扫描需要有效的非空处方".into());
    }
    if o.f_numbers.is_empty()
        || o.f_numbers.len() > 12
        || o.f_numbers
            .iter()
            .any(|f| !f.is_finite() || !(0.5..=128.).contains(f))
        || !(7..=33).contains(&o.sampling)
        || o.sampling.is_multiple_of(2)
    {
        return Err("光圈扫描：1–12 档 F 数（0.5–128），奇数采样 7–33".into());
    }
    let mut points = Vec::new();
    for &f in &o.f_numbers {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let mut point = AperturePoint {
            coverage: None,
            fields_rad: vec![],
            requested_f_number: f,
            actual_f_number: None,
            stop_radius_mm: None,
            image_distance_mm: p.surfaces.last().unwrap().thickness,
            fields_rms_um: vec![None; p.fields.len()],
            score_um: None,
            failed: 0,
            sampled: 0,
            aiming_failed: 0,
            blocked: 0,
            mtf: vec![],
            diagnostic: None,
        };
        let run = (|| -> Result<(), String> {
            let mut q = at_f_number(p, f)?;
            point.stop_radius_mm = Some(q.surfaces[q.stop].semi_aperture);
            point.actual_f_number = paraxial(&q, 587.5618)?.f_number;
            let mut focus_error = None;
            if o.refocus {
                // Every aperture starts from the same source, not the previous optimum.
                match autofocus(&q, cancel) {
                    Ok(candidate) => q = candidate.project,
                    Err(e) => focus_error = Some(e),
                }
            }
            point.image_distance_mm = q.surfaces.last().unwrap().thickness;
            let a = analyze(&q, o.sampling, cancel)?;
            point.coverage = a.coverage.clone();
            point.fields_rad = a.fields.iter().map(|f| f.field).collect();
            point.failed = a.spots.iter().map(|s| s.failed).sum();
            point.sampled = a.spots.iter().map(|s| s.sampled).sum();
            point.aiming_failed = a
                .spots
                .iter()
                .flat_map(|s| &s.failures)
                .filter(|r| r.stage == TraceStage::Aiming)
                .map(|r| r.count)
                .sum();
            point.blocked = a
                .spots
                .iter()
                .flat_map(|s| &s.failures)
                .filter(|r| r.status == RayStatus::Blocked)
                .map(|r| r.count)
                .sum();
            point.fields_rms_um = a.fields.iter().map(|v| v.polychromatic_rms_um).collect();
            if let Some(error) = focus_error {
                point.diagnostic = Some(error);
                return Ok(());
            }
            if point.failed == 0 && point.fields_rms_um.iter().all(Option::is_some) {
                point.score_um = Some(
                    (point
                        .fields_rms_um
                        .iter()
                        .map(|v| v.unwrap().powi(2))
                        .sum::<f64>()
                        / a.fields.len() as f64)
                        .sqrt(),
                );
            } else {
                point.diagnostic = Some("失败样本未计为零误差；该档不参与最佳几何 RMS 选择".into());
            }
            if o.mtf {
                for (i, &field) in point.fields_rad.iter().enumerate() {
                    if cancel.load(Ordering::Relaxed) {
                        return Err("Cancelled".into());
                    }
                    let result = wave::diffraction(
                        &q,
                        &wave::WaveOptions {
                            method: o.wave_method,
                            field_index: i,
                            wavelength_nm: None,
                            max_pupil_samples: 256,
                        },
                        cancel,
                    );
                    if let Some(c) = point.coverage.as_mut() {
                        if let Some(f) = c.fields.get_mut(i) {
                            let status = if result.is_ok() {
                                "converged"
                            } else {
                                "failed"
                            };
                            if o.wave_method == wave::WaveMethod::Fast {
                                if f.fast_mtf != "outside_model" {
                                    f.fast_mtf = status.into();
                                }
                            } else {
                                f.wide_mtf = status.into();
                            }
                        }
                    }
                    point.mtf.push(match result {
                        Ok(w) => {
                            let sample = |values: &[f64], freq: f64| {
                                let pos = freq / w.frequency_step_lp_mm;
                                let lo = pos.floor() as usize;
                                if lo + 1 >= values.len() {
                                    None
                                } else {
                                    Some(
                                        values[lo]
                                            + (values[lo + 1] - values[lo]) * (pos - lo as f64),
                                    )
                                }
                            };
                            ApertureMtf {
                                method: o.wave_method,
                                field,
                                values: Some([
                                    sample(&w.mtf_sagittal, 10.),
                                    sample(&w.mtf_tangential, 10.),
                                    sample(&w.mtf_sagittal, 30.),
                                    sample(&w.mtf_tangential, 30.),
                                ]),
                                quality: Some(w.quality),
                                error: None,
                            }
                        }
                        Err(e) => ApertureMtf {
                            method: o.wave_method,
                            field,
                            values: None,
                            quality: None,
                            error: Some(e),
                        },
                    });
                }
            }
            Ok(())
        })();
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        if let Err(e) = run {
            point.diagnostic = Some(e);
            point.score_um = None;
        }
        points.push(point);
    }
    let best_index = points
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.score_um.map(|s| (i, s)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|v| v.0);
    Ok(ApertureScan {
        source: p.clone(),
        options: o.clone(),
        points,
        best_index,
    })
}

#[derive(Debug, Serialize)]
pub struct PupilSample {
    pub target_mm: [f64; 2],
    pub ray: RayPath,
}
#[derive(Debug, Serialize)]
pub struct StopMap {
    pub field: f64,
    pub wavelength_nm: f64,
    pub radius_mm: f64,
    pub sampling: usize,
    pub samples: Vec<PupilSample>,
}
pub fn stop_map(
    p: &Project,
    field_index: usize,
    nm: f64,
    sampling: usize,
    cancel: &AtomicBool,
) -> Result<StopMap, String> {
    if !p.validate_optical().is_empty() {
        return Err("瞳面图需要有效的非空处方".into());
    }
    if !(7..=33).contains(&sampling) || sampling.is_multiple_of(2) {
        return Err("瞳面图采样须为 7–33 的奇数".into());
    }
    let field = *p.fields.get(field_index).ok_or("视场不存在")?;
    for material in &p.materials {
        material.index(nm)?;
    }
    let radius = p.surfaces[p.stop].semi_aperture;
    let mut aimer = aiming::PupilAimer::new(p, field, nm);
    let mut samples = Vec::new();
    for y in 0..sampling {
        for x in 0..sampling {
            if cancel.load(Ordering::Relaxed) {
                return Err("Cancelled".into());
            }
            let uv = [
                2. * x as f64 / (sampling - 1) as f64 - 1.,
                2. * y as f64 / (sampling - 1) as f64 - 1.,
            ];
            if uv[0].hypot(uv[1]) > 1. + 1e-12 {
                continue;
            }
            let target = [uv[0] * radius, uv[1] * radius];
            samples.push(PupilSample {
                target_mm: target,
                ray: aimer.aim(target, true),
            });
        }
    }
    Ok(StopMap {
        field,
        wavelength_nm: nm,
        radius_mm: radius,
        sampling,
        samples,
    })
}

#[derive(Debug, Deserialize)]
pub struct GroupMove {
    pub start: usize,
    pub end: usize,
    pub delta_mm: f64,
}
pub fn move_group(p: &Project, g: &GroupMove) -> Result<Project, String> {
    if g.start == 0
        || g.start > g.end
        || g.end >= p.surfaces.len()
        || !g.delta_mm.is_finite()
        || g.delta_mm.abs() > 1e4
    {
        return Err("镜组范围无效；第一面固定为坐标原点，首组不能整体平移".into());
    }
    if p.surfaces[g.start - 1].material != 0
        || p.surfaces[g.end].material != 0
        || p.surfaces[g.start].material == 0
    {
        return Err("编组必须从完整镜片前面开始，在空气中结束，不能拆开胶合面".into());
    }
    let mut q = p.clone();
    q.surfaces[g.start - 1].thickness += g.delta_mm;
    q.surfaces[g.end].thickness -= g.delta_mm;
    if q.surfaces[g.start - 1].thickness < 0. || q.surfaces[g.end].thickness <= 0. {
        return Err("平移会使边界空气间隔或像距非正".into());
    }
    let errors = q.validate();
    if !errors.is_empty() {
        return Err(serde_json::to_string(&errors).unwrap());
    }
    Ok(q)
}
