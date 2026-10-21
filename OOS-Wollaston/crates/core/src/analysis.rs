use crate::*;
use nalgebra::Matrix2;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Paraxial {
    pub efl: Option<f64>,
    pub bfl: Option<f64>,
    pub entrance_pupil_z: Option<f64>,
    pub entrance_pupil_radius: Option<f64>,
    pub exit_pupil_z: Option<f64>,
    pub exit_pupil_radius: Option<f64>,
    pub f_number: Option<f64>,
}
pub fn paraxial(p: &Project, nm: f64) -> Result<Paraxial, String> {
    if p.surfaces.is_empty() || p.stop >= p.surfaces.len() {
        return Err("Add a lens before optical analysis".into());
    }
    let mut m = Matrix2::identity();
    let mut pre = Matrix2::identity();
    let mut post = Matrix2::identity();
    let mut n = 1.;
    let zs = p.vertices();
    for (i, s) in p.surfaces.iter().enumerate() {
        if i == p.stop {
            pre = m;
        }
        let n2 = p.materials[s.material].index(nm)?;
        let r = Matrix2::new(1., 0., -(n2 - n) * s.curvature, 1.);
        m = r * m;
        if i >= p.stop {
            post = r * post;
        }
        n = n2;
        if i + 1 < p.surfaces.len() {
            let t = Matrix2::new(1., s.thickness / n, 0., 1.);
            m = t * m;
            if i >= p.stop {
                post = t * post;
            }
        }
    }
    let efl = if m[(1, 0)].abs() > 1e-14 {
        Some(-n / m[(1, 0)])
    } else {
        None
    };
    let bfl = if m[(1, 0)].abs() > 1e-14 {
        Some(-n * m[(0, 0)] / m[(1, 0)])
    } else {
        None
    };
    let ep = if pre[(0, 0)].abs() > 1e-14 {
        Some(pre[(0, 1)] / pre[(0, 0)])
    } else {
        None
    };
    let er = if pre[(0, 0)].abs() > 1e-14 {
        Some(p.surfaces[p.stop].semi_aperture / pre[(0, 0)].abs())
    } else {
        None
    };
    let xp = if post[(1, 1)].abs() > 1e-14 {
        Some(zs.last().unwrap() - n * post[(0, 1)] / post[(1, 1)])
    } else {
        None
    };
    let xr = if post[(1, 1)].abs() > 1e-14 {
        Some(p.surfaces[p.stop].semi_aperture / post[(1, 1)].abs())
    } else {
        None
    };
    Ok(Paraxial {
        efl,
        bfl,
        entrance_pupil_z: ep,
        entrance_pupil_radius: er,
        exit_pupil_z: xp,
        exit_pupil_radius: xr,
        f_number: efl.zip(er).map(|(f, r)| f.abs() / (2. * r)),
    })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Spot {
    pub field: f64,
    pub wavelength_nm: f64,
    pub weight: f64,
    pub points: Vec<[f64; 2]>,
    pub centroid: Option<[f64; 2]>,
    pub rms_um: Option<f64>,
    pub blocked_fraction: f64,
    pub failed: usize,
    pub sampled: usize,
    pub distortion_percent: Option<f64>,
    pub failures: Vec<RayFailure>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RayFailure {
    pub stage: TraceStage,
    pub status: RayStatus,
    pub surface: Option<usize>,
    pub reason: Option<String>,
    pub count: usize,
}
pub fn ray_failure_error(a: &Analysis) -> String {
    let failures:Vec<_>=a.spots.iter().filter(|s|s.failed>0).map(|s|serde_json::json!({
        "field_rad":s.field,"wavelength_nm":s.wavelength_nm,"failed":s.failed,"sampled":s.sampled,"causes":s.failures
    })).collect();
    serde_json::json!({"code":"ray_failures","failures":failures}).to_string()
}
pub fn ray_path_error(r: &RayPath) -> String {
    serde_json::json!({"code":"ray_failure","stage":r.stage,"status":r.status,"surface":r.surface,"wavelength_nm":r.wavelength_nm,"reason":r.aiming.as_ref().and_then(|a|a.failure.as_ref()),"target_mm":r.aiming.as_ref().map(|a|a.target_mm)}).to_string()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldSummary {
    pub field: f64,
    pub centroid: Option<[f64; 2]>,
    pub polychromatic_rms_um: Option<f64>,
    pub lateral_color_um: Option<f64>,
    pub sagittal_focus_offset_mm: Option<f64>,
    pub tangential_focus_offset_mm: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChromaticFocus {
    pub wavelength_nm: f64,
    pub paraxial_focus_offset_mm: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Analysis {
    pub coverage: Option<crate::fields::Coverage>,
    pub paraxial: Paraxial,
    pub spots: Vec<Spot>,
    pub fields: Vec<FieldSummary>,
    pub chromatic_focus: Vec<ChromaticFocus>,
    pub rays: Vec<RayPath>,
    pub diagnostics: Vec<Diagnostic>,
    pub sampling: usize,
    pub units: String,
    pub experimental_wave: Option<crate::wave::WaveResult>,
}
fn differential_focus(p: &Project, field: f64, axis: usize) -> Option<f64> {
    let eps = p.surfaces[p.stop].semi_aperture * 1e-3;
    let mut a = [0., 0.];
    a[axis] = eps;
    let mut b = [0., 0.];
    b[axis] = -eps;
    let r1 = aimed_ray(p, field, 587.5618, a);
    let r2 = aimed_ray(p, field, 587.5618, b);
    if r1.status != RayStatus::Complete || r2.status != RayStatus::Complete {
        return None;
    }
    let p1 = r1.points.last()?;
    let p2 = r2.points.last()?;
    let du = r1.direction[axis] / r1.direction[2] - r2.direction[axis] / r2.direction[2];
    if du.abs() < 1e-12 {
        None
    } else {
        Some(-(p1[axis] - p2[axis]) / du)
    }
}
pub fn summarize_field(field: f64, spots: &[Spot]) -> FieldSummary {
    let selected: Vec<_> = spots.iter().filter(|s| s.field == field).collect();
    let all_valid = selected
        .iter()
        .all(|s| s.centroid.is_some() && !s.points.is_empty());
    let weight = selected.iter().map(|s| s.weight).sum::<f64>();
    let centroid = if all_valid && weight > 0. {
        Some([
            selected
                .iter()
                .map(|s| s.weight * s.centroid.unwrap()[0])
                .sum::<f64>()
                / weight,
            selected
                .iter()
                .map(|s| s.weight * s.centroid.unwrap()[1])
                .sum::<f64>()
                / weight,
        ])
    } else {
        None
    };
    let rms = centroid.map(|c| {
        (selected
            .iter()
            .map(|s| {
                s.weight
                    * s.points
                        .iter()
                        .map(|v| (v[0] - c[0]).powi(2) + (v[1] - c[1]).powi(2))
                        .sum::<f64>()
                    / s.points.len() as f64
            })
            .sum::<f64>()
            / weight)
            .sqrt()
            * 1000.
    });
    let color = if all_valid && selected.len() > 1 {
        let ys: Vec<_> = selected.iter().map(|s| s.centroid.unwrap()[1]).collect();
        Some(
            (ys.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                - ys.iter().copied().fold(f64::INFINITY, f64::min))
                * 1000.,
        )
    } else {
        None
    };
    FieldSummary {
        field,
        centroid,
        polychromatic_rms_um: rms,
        lateral_color_um: color,
        sagittal_focus_offset_mm: None,
        tangential_focus_offset_mm: None,
    }
}
pub fn analyze(p: &Project, sampling: usize, cancel: &AtomicBool) -> Result<Analysis, String> {
    let errors = p.validate_optical();
    if !errors.is_empty() {
        return Err(serde_json::to_string(&errors).unwrap());
    }
    let resolved = crate::fields::resolve(p, cancel)?;
    let p = &resolved;
    let sampling = sampling.clamp(3, 65);
    let px = paraxial(p, 587.5618)?;
    let mut spots = Vec::new();
    let mut rays = Vec::new();
    let mut diagnostics = Vec::new();
    if p.object_distance.is_none() && px.bfl.is_some_and(|v| v <= 0.) {
        diagnostics.push(Diagnostic {
            surface: None,
            message: format!("VirtualImage|{:.6}", px.bfl.unwrap()),
        });
    }
    let radius = p.surfaces[p.stop].semi_aperture;
    for &field in &p.fields {
        for w in &p.wavelengths {
            let mut aimer = crate::aiming::PupilAimer::new(p, field, w.nm);
            let mut points = Vec::new();
            let mut failed = 0;
            let mut blocked = 0;
            let mut sampled = 0;
            let mut failures: Vec<RayFailure> = vec![];
            for iy in 0..sampling {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                for ix in 0..sampling {
                    if cancel.load(Ordering::Relaxed) {
                        return Err("Cancelled".into());
                    }
                    let x = 2. * ix as f64 / (sampling - 1) as f64 - 1.;
                    let y = 2. * iy as f64 / (sampling - 1) as f64 - 1.;
                    if x * x + y * y > 1. + 1e-12 {
                        continue;
                    }
                    sampled += 1;
                    let r = aimer.aim([x * radius, y * radius], true);
                    if r.status == RayStatus::Complete {
                        let q = r.points.last().unwrap();
                        points.push([q[0], q[1]]);
                    } else {
                        failed += 1;
                        if r.status == RayStatus::Blocked {
                            blocked += 1;
                        }
                        let reason = r.aiming.as_ref().and_then(|a| a.failure.clone());
                        if let Some(existing) = failures.iter_mut().find(|f| {
                            f.stage == r.stage
                                && f.status == r.status
                                && f.surface == r.surface
                                && f.reason == reason
                        }) {
                            existing.count += 1;
                        } else {
                            failures.push(RayFailure {
                                stage: r.stage.clone(),
                                status: r.status.clone(),
                                surface: r.surface,
                                reason,
                                count: 1,
                            });
                        }
                        let message = if r.stage == TraceStage::Aiming {
                            format!(
                                "AimingFailed|{}",
                                r.aiming
                                    .as_ref()
                                    .and_then(|a| a.failure.as_deref())
                                    .unwrap_or("target_unresolved")
                            )
                        } else {
                            format!("{:?}", r.status)
                        };
                        if !diagnostics
                            .iter()
                            .any(|d: &Diagnostic| d.surface == r.surface && d.message == message)
                        {
                            diagnostics.push(Diagnostic {
                                surface: r.surface,
                                message,
                            });
                        }
                    }
                    if ix == sampling / 2 && iy % ((sampling / 7).max(1)) == 0 {
                        rays.push(r);
                    }
                }
            }
            let centroid = if points.is_empty() {
                None
            } else {
                Some([
                    points.iter().map(|v| v[0]).sum::<f64>() / points.len() as f64,
                    points.iter().map(|v| v[1]).sum::<f64>() / points.len() as f64,
                ])
            };
            let rms_um = centroid.map(|c| {
                (points
                    .iter()
                    .map(|q| (q[0] - c[0]).powi(2) + (q[1] - c[1]).powi(2))
                    .sum::<f64>()
                    / points.len() as f64)
                    .sqrt()
                    * 1000.
            });
            let chief = aimer.aim([0., 0.], true);
            let distortion_percent = if p.object_distance.is_none()
                && field.abs() > 1e-10
                && chief.status == RayStatus::Complete
            {
                px.efl
                    .map(|f| 100. * (chief.points.last().unwrap()[1] / (f * field.tan()) - 1.))
            } else {
                None
            };
            spots.push(Spot {
                field,
                wavelength_nm: w.nm,
                weight: w.weight,
                points,
                centroid,
                rms_um,
                blocked_fraction: blocked as f64 / sampled as f64,
                failed,
                sampled,
                distortion_percent,
                failures,
            });
        }
    }
    let fields = p
        .fields
        .iter()
        .map(|&f| {
            let mut s = summarize_field(f, &spots);
            s.sagittal_focus_offset_mm = differential_focus(p, f, 0);
            s.tangential_focus_offset_mm = differential_focus(p, f, 1);
            s
        })
        .collect();
    let chromatic_focus = p
        .wavelengths
        .iter()
        .map(|w| ChromaticFocus {
            wavelength_nm: w.nm,
            paraxial_focus_offset_mm: paraxial(p, w.nm)
                .ok()
                .and_then(|a| a.bfl)
                .zip(px.bfl)
                .map(|(b, reference)| b - reference),
        })
        .collect();
    let mut a = Analysis {
        coverage: None,
        paraxial: px,
        spots,
        fields,
        chromatic_focus,
        rays,
        diagnostics,
        sampling,
        units: "length:mm; angle:rad; wavelength:nm; RMS:µm".into(),
        experimental_wave: None,
    };
    a.coverage = Some(crate::fields::coverage(p, &a));
    Ok(a)
}
