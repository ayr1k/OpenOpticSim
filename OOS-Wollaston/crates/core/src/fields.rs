//! Field specifications refer to chief-ray height on the actual, common flat sensor.
//! Height inversion follows the branch continuously from the axial chief; no
//! clipped or unresolved ray is silently used as a paraxial substitute.
use crate::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

pub const MAX_FIELD_RAD: f64 = 0.7;
pub const HEIGHT_TOL_MM: f64 = 0.002;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum FieldSpec {
    Sensor,
    ImageHeights { heights_mm: Vec<f64> },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldResolution {
    pub label: String,
    pub target_height_mm: Option<f64>,
    pub initial_angle_rad: Option<f64>,
    pub angle_rad: Option<f64>,
    pub actual_height_mm: Option<f64>,
    pub residual_mm: Option<f64>,
    pub chief_status: String,
    pub diagnostic: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldPlan {
    pub efl_mm: Option<f64>,
    pub reference_wavelength_nm: f64,
    pub geometry_limit_rad: f64,
    pub fields: Vec<FieldResolution>,
}
pub fn sensor_heights(p: &Project) -> [f64; 3] {
    [
        p.sensor[0].min(p.sensor[1]) / 2.,
        p.sensor[0].max(p.sensor[1]) / 2.,
        p.sensor[0].hypot(p.sensor[1]) / 2.,
    ]
}
pub fn reference_nm(p: &Project) -> f64 {
    p.wavelengths
        .iter()
        .min_by(|a, b| (a.nm - 587.5618).abs().total_cmp(&(b.nm - 587.5618).abs()))
        .map_or(587.5618, |w| w.nm)
}
fn height(p: &Project, angle: f64, nm: f64) -> Result<f64, String> {
    let r = aimed_ray_with_clip(p, angle, nm, [0., 0.], false);
    if r.status != RayStatus::Complete {
        return Err(ray_path_error(&r));
    }
    Ok(r.points.last().unwrap()[1])
}
fn invert(p: &Project, target: f64, nm: f64, cancel: &AtomicBool) -> Result<f64, String> {
    if target.abs() < 1e-12 {
        height(p, 0., nm)?;
        return Ok(0.);
    }
    let sign = target.signum();
    let goal = target.abs();
    let mut lo = 0.;
    let mut previous = height(p, 0., nm)? * sign;
    // Small angular steps prevent jumping over unresolved or folded chief branches.
    for i in 1..=70 {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let hi = MAX_FIELD_RAD * i as f64 / 70.;
        let h = height(p, hi * sign, nm)? * sign;
        if h < previous - 1e-6 {
            return Err("实际主光线像高不单调，无法确认连接轴上的唯一视场分支".into());
        }
        if h >= goal {
            let mut upper = hi;
            for _ in 0..45 {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                let mid = (lo + upper) / 2.;
                let actual = height(p, mid * sign, nm)? * sign;
                if (actual - goal).abs() < 1e-7 {
                    return Ok(mid * sign);
                }
                if actual < goal {
                    lo = mid;
                } else {
                    upper = mid;
                }
            }
            return Ok((lo + upper) / 2. * sign);
        }
        lo = hi;
        previous = h;
    }
    Err(format!(
        "目标像高 {target:.3} mm 未在 ±{:.2}° 内找到；不得以理想视场代替",
        MAX_FIELD_RAD.to_degrees()
    ))
}
pub fn plan(p: &Project, cancel: &AtomicBool) -> Result<FieldPlan, String> {
    let nm = reference_nm(p);
    let f = paraxial(p, nm)?.efl.filter(|f| f.abs() > 1e-10);
    let targets: Vec<(String, Option<f64>, Option<f64>)> = match &p.field_spec {
        Some(FieldSpec::Sensor) => std::iter::once(("中心".into(), Some(0.), None))
            .chain(
                sensor_heights(p)
                    .into_iter()
                    .zip(["短边中点", "长边中点", "画面角落"])
                    .map(|(h, l)| (l.into(), Some(h), None)),
            )
            .collect(),
        Some(FieldSpec::ImageHeights { heights_mm }) => heights_mm
            .iter()
            .enumerate()
            .map(|(i, &h)| (format!("像高 {}", i + 1), Some(h), None))
            .collect(),
        None => p
            .fields
            .iter()
            .enumerate()
            .map(|(i, &a)| (format!("视场 {}", i + 1), None, Some(a)))
            .collect(),
    };
    let mut fields = Vec::new();
    for (label, target, angle) in targets {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let initial = target.and_then(|h| f.map(|f| (h / f).atan())).or(angle);
        let solved = angle
            .map(Ok)
            .unwrap_or_else(|| invert(p, target.unwrap(), nm, cancel));
        let mut r = FieldResolution {
            label,
            target_height_mm: target,
            initial_angle_rad: initial,
            angle_rad: None,
            actual_height_mm: None,
            residual_mm: None,
            chief_status: "unresolved".into(),
            diagnostic: None,
        };
        match solved {
            Err(e) => {
                if e == "Cancelled" {
                    return Err(e);
                }
                r.diagnostic = Some(e);
            }
            Ok(a) => {
                r.angle_rad = Some(a);
                match height(p, a, nm) {
                    Ok(h) => {
                        r.actual_height_mm = Some(h);
                        r.residual_mm = target.map(|t| h - t);
                    }
                    Err(e) => r.diagnostic = Some(e),
                }
                let chief = aimed_ray(p, a, nm, [0., 0.]);
                r.chief_status = serde_json::to_value(&chief.status)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .into();
                if chief.status != RayStatus::Complete {
                    r.diagnostic = Some(ray_path_error(&chief));
                }
            }
        }
        fields.push(r);
    }
    Ok(FieldPlan {
        efl_mm: f,
        reference_wavelength_nm: nm,
        geometry_limit_rad: MAX_FIELD_RAD,
        fields,
    })
}
pub fn resolve(p: &Project, cancel: &AtomicBool) -> Result<Project, String> {
    if p.field_spec.is_none() {
        return Ok(p.clone());
    }
    let plan = plan(p, cancel)?;
    let mut out = p.clone();
    out.fields = plan
        .fields
        .iter()
        .map(|r| {
            r.angle_rad.ok_or_else(|| {
                format!(
                    "{}：{}",
                    r.label,
                    r.diagnostic.as_deref().unwrap_or("目标像高无法求解")
                )
            })
        })
        .collect::<Result<_, _>>()?;
    // The resolved copy is internal only. Keeping specs would re-solve recursively.
    out.field_spec = None;
    Ok(out)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoverageField {
    pub angle_rad: f64,
    pub chief_height_mm: Option<f64>,
    pub sampled: usize,
    pub blocked: usize,
    pub unresolved: usize,
    pub other_failed: usize,
    pub geometric_fraction: f64,
    pub fast_mtf: String,
    pub wide_mtf: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoverageCheckpoint {
    pub label: String,
    pub height_mm: f64,
    pub ideal_angle_rad: Option<f64>,
    pub tested_field_index: Option<usize>,
    pub status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Coverage {
    pub sensor_mm: [f64; 2],
    pub half_diagonal_mm: f64,
    pub max_tested_height_mm: Option<f64>,
    pub reference_wavelength_nm: f64,
    pub sampling: usize,
    pub image_z_mm: f64,
    pub focus_condition: String,
    pub fields: Vec<CoverageField>,
    pub checkpoints: Vec<CoverageCheckpoint>,
    pub warnings: Vec<String>,
}
pub fn coverage(p: &Project, a: &Analysis) -> Coverage {
    let nm = reference_nm(p);
    let fields: Vec<_> = p
        .fields
        .iter()
        .map(|&f| {
            let chief = aimed_ray(p, f, nm, [0., 0.]);
            let h = if chief.status == RayStatus::Complete {
                chief.points.last().map(|q| q[0].hypot(q[1]))
            } else {
                None
            };
            let spots: Vec<_> = a.spots.iter().filter(|s| s.field == f).collect();
            let sampled = spots.iter().map(|s| s.sampled).sum::<usize>();
            let blocked = spots
                .iter()
                .flat_map(|s| &s.failures)
                .filter(|e| e.status == RayStatus::Blocked)
                .map(|e| e.count)
                .sum::<usize>();
            let unresolved = spots
                .iter()
                .flat_map(|s| &s.failures)
                .filter(|e| e.stage == TraceStage::Aiming)
                .map(|e| e.count)
                .sum::<usize>();
            let failed = spots.iter().map(|s| s.failed).sum::<usize>();
            CoverageField {
                angle_rad: f,
                chief_height_mm: h,
                sampled,
                blocked,
                unresolved,
                other_failed: failed - blocked - unresolved,
                geometric_fraction: 1. - failed as f64 / sampled.max(1) as f64,
                fast_mtf: if f.abs() > 0.15 {
                    "outside_model"
                } else {
                    "not_calculated"
                }
                .into(),
                wide_mtf: "not_calculated".into(),
            }
        })
        .collect();
    let checkpoints = sensor_heights(p)
        .into_iter()
        .zip(["短边中点", "长边中点", "画面角落"])
        .map(|(h, label)| {
            let index = fields.iter().position(|f| {
                f.chief_height_mm
                    .is_some_and(|v| (v - h).abs() <= HEIGHT_TOL_MM)
            });
            let status = index.map_or("untested", |i| {
                let f = &fields[i];
                if f.unresolved + f.other_failed > 0 {
                    "unresolved"
                } else if f.blocked > 0 {
                    "clipped"
                } else {
                    "sampled_clear"
                }
            });
            CoverageCheckpoint {
                label: label.into(),
                height_mm: h,
                ideal_angle_rad: a.paraxial.efl.map(|f| (h / f.abs()).atan()),
                tested_field_index: index,
                status: status.into(),
            }
        })
        .collect::<Vec<_>>();
    let mut warnings=vec!["覆盖结论仅针对列出的视场与瞳采样，不证明整个像圈连续通光；几何采样比例不是精确相对照度。MTF 必须单独计算并通过适用性与收敛检查。".into()];
    for c in &checkpoints {
        if c.status != "sampled_clear" {
            warnings.push(format!(
                "{} {:.3} mm：{}",
                c.label,
                c.height_mm,
                match c.status.as_str() {
                    "untested" => "未验证（邻近或更大的视场不能代替该检查点）",
                    "clipped" => "已测试但存在物理遮挡",
                    _ => "求解或追迹未完成",
                }
            ));
        }
    }
    if fields.iter().any(|f| f.angle_rad.abs() > 0.15) {
        warnings.push("部分外场超出快速 MTF 模型 ±8.59°；请单独运行离轴平面像面模型，未计算或未收敛时仍为未验证。".into());
    }
    Coverage {
        sensor_mm: p.sensor,
        half_diagonal_mm: p.sensor[0].hypot(p.sensor[1]) / 2.,
        max_tested_height_mm: fields
            .iter()
            .filter(|f| f.sampled > f.blocked + f.unresolved + f.other_failed)
            .filter_map(|f| f.chief_height_mm)
            .reduce(f64::max),
        reference_wavelength_nm: nm,
        sampling: a.sampling,
        image_z_mm: p.image_z(),
        focus_condition: "common_flat_sensor".into(),
        fields,
        checkpoints,
        warnings,
    }
}
