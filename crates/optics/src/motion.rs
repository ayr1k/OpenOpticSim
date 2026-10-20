//! Rigid groups in a fixed sensor/object reference frame. No optical kernel changes.
use crate::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MovingGroup {
    pub start: usize,
    pub end: usize,
    pub min_mm: f64,
    pub max_mm: f64,
    pub locked: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MotionOptions {
    pub groups: Vec<MovingGroup>,
    pub positions_mm: Vec<f64>,
    /// Distance from reference first vertex; sensor and object remain fixed during motion.
    pub object_distance: Option<f64>,
    pub min_air_mm: f64,
    pub target_f_number: Option<f64>,
    pub solve_group: Option<usize>,
    pub sampling: usize,
    pub criterion: FocusCriterion,
    pub max_rms_um: f64,
}
#[derive(Debug, Serialize)]
pub struct MotionPoint {
    pub positions_mm: Vec<f64>,
    pub score_um: Option<f64>,
    pub diagnostic: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct MotionResult {
    pub project: Project,
    pub positions_mm: Vec<f64>,
    pub origin_shift_mm: f64,
    pub sensor_reference_mm: f64,
    pub analysis: Analysis,
    pub score_um: Option<f64>,
    pub accepted: bool,
    pub boundary_best: bool,
    pub points: Vec<MotionPoint>,
    pub diagnostic: String,
}
pub fn placed(p: &Project, o: &MotionOptions) -> Result<(Project, f64), String> {
    if o.groups.is_empty()
        || o.groups.len() > 20
        || o.positions_mm.len() != o.groups.len()
        || !o.min_air_mm.is_finite()
        || o.min_air_mm < 0.
        || o.object_distance.is_some_and(|d| !d.is_finite() || d <= 1.)
    {
        return Err("镜组配置或参考物距无效".into());
    }
    let n = p.surfaces.len();
    let mut shifts = vec![0.; n];
    let mut occupied = vec![false; n];
    for (g, &d) in o.groups.iter().zip(&o.positions_mm) {
        if g.start > g.end
            || g.end >= n
            || ![d, g.min_mm, g.max_mm].iter().all(|x| x.is_finite())
            || g.min_mm > g.max_mm
            || g.min_mm.abs() > 1e4
            || g.max_mm.abs() > 1e4
            || d < g.min_mm
            || d > g.max_mm
            || (g.locked && d.abs() > 1e-12)
        {
            return Err("镜组超出行程、被锁定或范围无效".into());
        }
        if p.surfaces[g.start].material == 0
            || p.surfaces[g.end].material != 0
            || (g.start > 0 && p.surfaces[g.start - 1].material != 0)
        {
            return Err("镜组必须包含完整镜片，不能拆开胶合面".into());
        }
        for i in g.start..=g.end {
            if occupied[i] {
                return Err("镜组范围重叠".into());
            }
            occupied[i] = true;
            shifts[i] = d;
        }
    }
    let origin = shifts[0];
    let mut q = p.clone();
    for i in 0..n {
        q.surfaces[i].thickness += shifts.get(i + 1).copied().unwrap_or(0.) - shifts[i];
    }
    q.object_distance = o.object_distance.map(|d| d + origin);
    if let Some(d) = o.object_distance {
        if q.field_spec.is_none() {
            q.fields = p
                .fields
                .iter()
                .map(|f| (d * f.tan() / (d + origin)).atan())
                .collect();
        }
    }
    let errors = q.validate();
    if !errors.is_empty() {
        return Err(serde_json::to_string(&errors).unwrap());
    }
    let geometry = geometry::inspect(&q);
    if geometry
        .spans
        .iter()
        .any(|s| !s.glass && !geometry::clearance_at_least(&q, s, o.min_air_mm))
        || q.surfaces[n - 1].thickness < o.min_air_mm
    {
        return Err("镜组移动违反最小机械空气间隙".into());
    }
    if let Some(f) = o.target_f_number {
        if !f.is_finite() || !(0.5..=128.).contains(&f) {
            return Err("目标 F 数须在 0.5–128 之间".into());
        }
        let current = paraxial(&q, 587.5618)?
            .f_number
            .ok_or("入瞳未定义，不能设置目标 F 数")?;
        let radius = q.surfaces[q.stop].semi_aperture * current / f;
        if radius > p.surfaces[p.stop].semi_aperture + 1e-10 {
            return Err("目标 F 数需要超过参考处方有效口径的光阑".into());
        }
        q.surfaces[q.stop].semi_aperture = radius;
    }
    Ok((q, origin))
}
fn score(a: &Analysis, criterion: &FocusCriterion) -> Option<f64> {
    if a.spots.is_empty() || a.spots.iter().any(|s| s.failed > 0 || s.sampled == 0) {
        return None;
    }
    let values: Option<Vec<f64>> = a.fields.iter().map(|f| f.polychromatic_rms_um).collect();
    let values = values?;
    if values.is_empty() {
        return None;
    }
    match criterion {
        FocusCriterion::Center => {
            a.fields
                .iter()
                .find(|f| f.field.abs() < 1e-12)?
                .polychromatic_rms_um
        }
        FocusCriterion::Balanced => {
            Some((values.iter().map(|v| v * v).sum::<f64>() / values.len() as f64).sqrt())
        }
    }
}
pub fn run(p: &Project, o: &MotionOptions, cancel: &AtomicBool) -> Result<MotionResult, String> {
    if !(7..=33).contains(&o.sampling)
        || o.sampling.is_multiple_of(2)
        || !o.max_rms_um.is_finite()
        || o.max_rms_um <= 0.
    {
        return Err("采样须为 7–33 的奇数，RMS 验收上限须为正数".into());
    }
    // A seed can collide even though another compensator position is feasible.
    // Validate vector shape before indexing; each trial still receives full validation.
    if o.positions_mm.len() != o.groups.len() || o.positions_mm.iter().any(|x| !x.is_finite()) {
        return Err("镜组位置数量或数值无效".into());
    }
    if o.solve_group.is_none() {
        placed(p, o)?;
    }
    let mut options = o.clone();
    let mut points = Vec::new();
    let mut best: Option<(f64, Vec<f64>)> = None;
    if let Some(index) = o.solve_group {
        let g = o.groups.get(index).ok_or("未选择对焦/补偿组")?;
        if g.locked || g.max_mm <= g.min_mm {
            return Err("对焦/补偿组被锁定或没有行程".into());
        }
        let (mut lo, mut hi) = (g.min_mm, g.max_mm);
        // Coarse full-travel scan followed by bounded refinement around the best sample.
        for pass in 0..4 {
            let count = if pass == 0 { 25 } else { 9 };
            let step = (hi - lo) / (count - 1) as f64;
            for i in 0..count {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                options.positions_mm[index] = lo + step * i as f64;
                let evaluated =
                    placed(p, &options).and_then(|(q, _)| analyze(&q, o.sampling, cancel));
                let (s, diagnostic) = match evaluated {
                    Ok(a) => {
                        let s = score(&a, &o.criterion);
                        (
                            s,
                            s.is_none()
                                .then(|| "存在遮挡、瞳瞄准失败或无有效视场；不计为零误差".into()),
                        )
                    }
                    Err(e) => (None, Some(e)),
                };
                if let Some(s) = s {
                    if best.as_ref().is_none_or(|b| s < b.0) {
                        best = Some((s, options.positions_mm.clone()));
                    }
                }
                points.push(MotionPoint {
                    positions_mm: options.positions_mm.clone(),
                    score_um: s,
                    diagnostic,
                });
            }
            let Some((_, positions)) = &best else {
                break;
            };
            lo = (positions[index] - step).max(g.min_mm);
            hi = (positions[index] + step).min(g.max_mm);
        }
        options.positions_mm = best
            .ok_or("行程内没有通过全部声明视场采样的位置；请检查机械间隙、通光或扩大行程")?
            .1;
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".into());
    }
    let (q, origin) = placed(p, &options)?;
    let a = analyze(&q, o.sampling, cancel)?;
    let s = score(&a, &o.criterion);
    let boundary = o.solve_group.is_some_and(|i| {
        let g = &o.groups[i];
        let d = options.positions_mm[i];
        (d - g.min_mm).abs().min((d - g.max_mm).abs()) < 1e-5
    });
    let accepted = s.is_some_and(|v| v <= o.max_rms_um) && !boundary;
    Ok(MotionResult {
        project: q,
        positions_mm: options.positions_mm,
        origin_shift_mm: origin,
        sensor_reference_mm: p.image_z(),
        analysis: a,
        score_um: s,
        accepted,
        boundary_best: boundary,
        points,
        diagnostic: if boundary {
            "最优位置位于行程边界，尚未确认合焦"
        } else if accepted {
            "声明采样与 RMS 上限通过；非全局最优或制造质量保证"
        } else {
            "未达到声明的 RMS 上限或存在失败光线"
        }
        .into(),
    })
}
