use crate::*;
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub project: Project,
    pub before_rms_um: f64,
    pub after_rms_um: f64,
    pub iterations: usize,
    pub note: String,
    pub accepted: bool,
    pub checks: Vec<ConstraintCheck>,
    pub sampling: usize,
    pub report: Option<OptimizationReport>,
}
fn merit(p: &Project, cancel: &AtomicBool) -> Result<f64, String> {
    let a = analyze(p, 9, cancel)?;
    if a.spots.iter().any(|s| s.failed > 0) {
        return Err(ray_failure_error(&a));
    }
    let values: Result<Vec<_>, _> = a
        .fields
        .iter()
        .map(|f| f.polychromatic_rms_um.ok_or("No valid rays"))
        .collect();
    let values = values?;
    Ok((values.iter().map(|r| r * r).sum::<f64>() / values.len() as f64).sqrt())
}
pub fn autofocus(p: &Project, cancel: &AtomicBool) -> Result<Candidate, String> {
    if !p.validate_optical().is_empty() {
        return Err("Invalid project".into());
    }
    if p.object_distance.is_none() {
        if let Some(bfl) = paraxial(p, 587.5618)?.bfl.filter(|v| *v <= 0.) {
            return Err(serde_json::json!({"code":"virtual_image","bfl_mm":bfl}).to_string());
        }
    }
    let before = merit(p, cancel)?;
    let last = p.surfaces.len() - 1;
    let current = p.surfaces[last].thickness;
    let mut lo = (current * 0.2).max(0.01);
    let mut hi = (current * 3.).max(20.);
    if let Some(bfl) = paraxial(p, 587.5618)?.bfl.filter(|v| *v > 0.) {
        lo = lo.min(bfl * 0.5).max(0.01);
        hi = hi.max(bfl * 1.5);
    }
    let mut iterations = 0;
    // Convex RMS^2 in image distance for a fixed set of emerging rays.
    let mut candidate = p.clone();
    if p.field_spec.is_some() {
        // Image-height fields change object angle when the sensor moves, so
        // RMS² need not be a single quadratic. Bracket a sampled local minimum
        // before the local focus search; do not claim global focus optimality.
        let grid: Vec<_> = (0..=24).map(|i| lo + (hi - lo) * i as f64 / 24.).collect();
        let mut best_index = None;
        let mut score = f64::INFINITY;
        for (i, &z) in grid.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err("Cancelled".into());
            }
            candidate.surfaces[last].thickness = z;
            let value = merit(&candidate, cancel).unwrap_or(f64::INFINITY);
            if value < score {
                score = value;
                best_index = Some(i);
            }
        }
        if let Some(i) = best_index {
            lo = grid[i.saturating_sub(1)];
            hi = grid[(i + 1).min(24)];
        }
        candidate = p.clone();
    }
    for _ in 0..38 {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let a = lo + (hi - lo) / 3.;
        let b = hi - (hi - lo) / 3.;
        candidate.surfaces[last].thickness = a;
        let fa = merit(&candidate, cancel).unwrap_or(f64::INFINITY);
        candidate.surfaces[last].thickness = b;
        let fb = merit(&candidate, cancel).unwrap_or(f64::INFINITY);
        if fa < fb {
            hi = b;
        } else {
            lo = a;
        }
        iterations += 1;
    }
    candidate.surfaces[last].thickness = (lo + hi) / 2.;
    let after = merit(&candidate, cancel)?;
    if after >= before {
        candidate = p.clone();
    }
    Ok(Candidate {
        project: candidate,
        before_rms_um: before,
        after_rms_um: after.min(before),
        iterations,
        note: "Local RMS focus, fixed lens surfaces. Candidate requires explicit acceptance."
            .into(),
        accepted: true,
        checks: vec![],
        sampling: 9,
        report: None,
    })
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Variable {
    Curvature { surface: usize },
    Thickness { surface: usize },
    Aperture { surface: usize },
    Asphere { surface: usize, coefficient: usize },
}
pub(crate) fn value(p: &Project, v: &Variable) -> Option<f64> {
    match *v {
        Variable::Curvature { surface } => p.surfaces.get(surface).map(|s| s.curvature),
        Variable::Thickness { surface } => p.surfaces.get(surface).map(|s| s.thickness),
        Variable::Aperture { surface } => {
            (surface == p.stop).then(|| p.surfaces[surface].semi_aperture)
        }
        Variable::Asphere {
            surface,
            coefficient,
        } => p
            .surfaces
            .get(surface)
            .and_then(|s| s.asphere.get(coefficient).copied()),
    }
}
pub(crate) fn set(p: &mut Project, v: &Variable, x: f64) {
    match *v {
        Variable::Curvature { surface } => p.surfaces[surface].curvature = x,
        Variable::Thickness { surface } => p.surfaces[surface].thickness = x,
        Variable::Aperture { .. } => aperture::set_stop_radius(p, x),
        Variable::Asphere {
            surface,
            coefficient,
        } => p.surfaces[surface].asphere[coefficient] = x,
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisPreset {
    Center,
    #[default]
    Balanced,
    Edge,
}
pub fn field_weights(fields: &[f64], preset: AnalysisPreset) -> Vec<f64> {
    let max = fields.iter().map(|f| f.abs()).fold(0., f64::max).max(1e-12);
    let mut weights: Vec<_> = fields
        .iter()
        .map(|f| {
            let t = f.abs() / max;
            match preset {
                AnalysisPreset::Center => 1. + 3. * (1. - t).powi(2),
                AnalysisPreset::Balanced => 1.,
                AnalysisPreset::Edge => 1. + 3. * t * t,
            }
        })
        .collect();
    let sum: f64 = weights.iter().sum();
    weights.iter_mut().for_each(|w| *w /= sum);
    weights
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VariableSetting {
    pub variable: Variable,
    pub min: Option<f64>,
    pub max: Option<f64>,
    #[serde(default)]
    pub locked: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OptimizationOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<crate::quality::QualityGuards>,
    #[serde(default)]
    pub staged: bool,
    #[serde(default)]
    pub refocus_each_stage: bool,
    pub variables: Vec<VariableSetting>,
    #[serde(default)]
    pub preset: AnalysisPreset,
    #[serde(default = "default_iterations")]
    pub max_iterations: usize,
}
fn default_iterations() -> usize {
    25
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OptimizationProgress {
    pub iteration: usize,
    pub max_iterations: usize,
    pub merit: f64,
    pub weighted_rms_um: f64,
    pub efl_mm: Option<f64>,
    pub total_length_mm: f64,
    pub step_accepted: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OptimizationReport {
    pub focus_condition: String,
    pub before_coverage: Option<crate::fields::Coverage>,
    pub after_coverage: Option<crate::fields::Coverage>,
    pub stages: Vec<StageReport>,
    pub preset: AnalysisPreset,
    pub field_weights: Vec<f64>,
    pub before_fields_rms_um: Vec<Option<f64>>,
    pub after_fields_rms_um: Vec<Option<f64>>,
    pub before_weighted_rms_um: Option<f64>,
    pub after_weighted_rms_um: Option<f64>,
    pub variables: Vec<VariableSetting>,
    pub history: Vec<OptimizationProgress>,
    pub stop_reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageReport {
    pub field_scale: f64,
    pub original_fields_rms_um: Vec<Option<f64>>,
    pub candidate_fields_rms_um: Vec<Option<f64>>,
    pub accepted: bool,
    pub original_coverage: Option<crate::fields::Coverage>,
    pub candidate_coverage: Option<crate::fields::Coverage>,
}
fn weighted_rms(a: &Analysis, weights: &[f64]) -> Option<f64> {
    a.fields
        .iter()
        .zip(weights)
        .map(|(f, w)| f.polychromatic_rms_um.map(|r| w * r * r))
        .collect::<Option<Vec<_>>>()
        .map(|v| v.iter().sum::<f64>().sqrt())
}
struct Evaluation {
    residual: DVector<f64>,
    analysis: Analysis,
}
fn residuals(
    p: &Project,
    constraints: &Constraints,
    weights: &[f64],
    quality: Option<&crate::quality::QualityGuards>,
    baseline: &Analysis,
    cancel: &AtomicBool,
) -> Result<Evaluation, String> {
    let a = analyze(p, 7, cancel)?;
    let mut out = Vec::new();
    let spectral_weight = p.wavelengths.iter().map(|w| w.weight).sum::<f64>();
    let allow_clipping = quality
        .and_then(|g| g.min_geometric_fraction)
        .is_some_and(|f| f < 1.);
    for spot in &a.spots {
        if spot.failed > 0
            && (!allow_clipping || spot.failures.iter().any(|f| f.status != RayStatus::Blocked))
        {
            return Err("Clipped or failed rays".into());
        }
        if allow_clipping {
            continue;
        }
        let field_index = a
            .fields
            .iter()
            .position(|f| f.field == spot.field)
            .ok_or("Missing field")?;
        let c = a
            .fields
            .iter()
            .find(|f| f.field == spot.field)
            .and_then(|f| f.centroid)
            .ok_or("No rays")?;
        let scale =
            (weights[field_index] * spot.weight / spectral_weight / spot.points.len() as f64)
                .sqrt()
                / 0.01;
        for point in &spot.points {
            out.push((point[0] - c[0]) * scale);
            out.push((point[1] - c[1]) * scale);
        }
    }
    if allow_clipping {
        for (f, w) in a.fields.iter().zip(weights) {
            out.push(f.polychromatic_rms_um.ok_or("No valid field rays")? / 10. * w.sqrt());
        }
    }
    let f = a.paraxial.efl.ok_or("Afocal system")?;
    out.push((f - constraints.target_efl_mm) / constraints.efl_tolerance_mm);
    out.extend(constraints::penalties(p, constraints));
    if let Some(g) = quality {
        out.extend(crate::quality::penalties(&a, baseline, g));
    }
    Ok(Evaluation {
        residual: DVector::from_vec(out),
        analysis: a,
    })
}
pub fn optimize(
    p: &Project,
    variables: &[Variable],
    target_efl: f64,
    cancel: &AtomicBool,
) -> Result<Candidate, String> {
    optimize_constrained(
        p,
        variables,
        &Constraints {
            target_efl_mm: target_efl,
            efl_tolerance_mm: target_efl.abs().max(1.) * 0.01,
            max_length_mm: p.image_z(),
            min_center_mm: 0.,
            min_edge_mm: 0.,
            min_air_mm: 0.,
            design: None,
        },
        cancel,
    )
}
pub fn optimize_constrained(
    p: &Project,
    variables: &[Variable],
    constraints: &Constraints,
    cancel: &AtomicBool,
) -> Result<Candidate, String> {
    optimize_configured(
        p,
        &OptimizationOptions {
            quality: None,
            staged: false,
            refocus_each_stage: false,
            variables: variables
                .iter()
                .cloned()
                .map(|variable| VariableSetting {
                    variable,
                    min: None,
                    max: None,
                    locked: false,
                })
                .collect(),
            preset: AnalysisPreset::Balanced,
            max_iterations: 25,
        },
        constraints,
        cancel,
        &mut |_| {},
    )
}
pub fn optimize_configured(
    p: &Project,
    options: &OptimizationOptions,
    constraints: &Constraints,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(&OptimizationProgress),
) -> Result<Candidate, String> {
    if options.staged {
        return staged_optimize(p, options, constraints, cancel, progress);
    }
    constraints.validate()?;
    if !p.validate_optical().is_empty() {
        if p.validate_structure().is_empty() && !p.surfaces.is_empty() {
            let mut repaired = p.clone();
            // Bounds/references are checked by the recursive call. Reject malformed
            // settings before any restoration can index a non-existent surface.
            if options.variables.iter().all(|s| {
                value(p, &s.variable).is_some()
                    && s.min.is_none_or(|v| v.is_finite())
                    && s.max.is_none_or(|v| v.is_finite())
                    && !matches!((s.min,s.max),(Some(a),Some(b)) if a>=b)
            }) {
                crate::restoration::restore(&mut repaired, &options.variables, constraints);
                if repaired.validate_optical().is_empty() {
                    let mut result =
                        optimize_configured(&repaired, options, constraints, cancel, progress)?;
                    result.note.push_str(
                        " 原始几何无效，先在允许变量范围内恢复；前后 RMS 以修复后处方为起点。",
                    );
                    return Ok(result);
                }
            }
        }
        return Err("Invalid project".into());
    }
    if options.max_iterations == 0 || options.max_iterations > 100 || options.variables.len() > 100
    {
        return Err("优化迭代数须为 1–100；变量表最多 100 行。".into());
    }
    for (i, setting) in options.variables.iter().enumerate() {
        let x = value(p, &setting.variable)
            .ok_or("优化变量引用了不存在的面或系数，请重新选择变量。")?;
        if setting.min.is_some_and(|v| !v.is_finite())
            || setting.max.is_some_and(|v| !v.is_finite())
            || matches!((setting.min,setting.max),(Some(a),Some(b)) if a >= b)
            || setting.min.is_some_and(|v| x < v)
            || setting.max.is_some_and(|v| x > v)
            || options.variables[..i]
                .iter()
                .any(|s| s.variable == setting.variable)
        {
            return Err(format!(
                "变量第 {} 行：上下限无效、当前值超界或变量重复。",
                i + 1
            ));
        }
    }
    let settings: Vec<_> = options.variables.iter().filter(|s| !s.locked).collect();
    if settings.is_empty() || settings.len() > 32 {
        return Err("请选择 1–32 个未锁定的优化变量。".into());
    }
    let variables: Vec<_> = settings.iter().map(|s| &s.variable).collect();
    let mut recoverable = p.clone();
    crate::restoration::restore(&mut recoverable, &options.variables, constraints);
    if crate::fields::resolve(p, cancel).is_err()
        || checked_analysis(p, options.quality.as_ref(), cancel).is_err()
    {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        if recoverable.validate_optical().is_empty()
            && checked_analysis(&recoverable, options.quality.as_ref(), cancel).is_ok()
        {
            let mut c = optimize_configured(&recoverable, options, constraints, cancel, progress)?;
            c.note
                .push_str(" 原始追迹失败，先恢复约束；前后 RMS 以恢复后的处方为起点。");
            return Ok(c);
        }
    }
    let resolved = crate::fields::resolve(p, cancel)?;
    if let Some(g) = &options.quality {
        g.validate(resolved.fields.len())?;
    }
    let weights = field_weights(&resolved.fields, options.preset);
    let before_analysis = checked_analysis(p, options.quality.as_ref(), cancel)?;
    let before = balanced_rms(&before_analysis).ok_or("No valid rays")?;
    let mut best = p.clone();
    crate::restoration::restore(&mut best, &options.variables, constraints);
    // Keep the original analysis as the quality-protection baseline.
    if !best.validate_optical().is_empty()
        || checked_analysis(&best, options.quality.as_ref(), cancel).is_err()
    {
        best = p.clone();
    }
    let baseline = analyze(p, 7, cancel)?;
    let mut eval = residuals(
        &best,
        constraints,
        &weights,
        options.quality.as_ref(),
        &baseline,
        cancel,
    )?;
    let mut feasible: Option<(Project, Analysis)> = None;
    if final_checks(p, options, constraints, &before_analysis, &before_analysis)
        .iter()
        .all(|c| c.passed)
    {
        feasible = Some((p.clone(), before_analysis.clone()));
    }
    let mut damping = 0.01;
    let mut iterations = 0;
    let mut history = vec![];
    let mut record = |iteration, eval: &Evaluation, model: &Project, step_accepted| {
        let event = OptimizationProgress {
            iteration,
            max_iterations: options.max_iterations,
            merit: eval.residual.norm_squared(),
            weighted_rms_um: weighted_rms(&eval.analysis, &weights).unwrap(),
            efl_mm: eval.analysis.paraxial.efl,
            total_length_mm: model.image_z(),
            step_accepted,
        };
        progress(&event);
        history.push(event);
    };
    record(0, &eval, &best, true);
    let mut stop_reason = "达到迭代上限".to_string();
    let scales: Vec<f64> = variables
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let floor = match v {
                Variable::Curvature { .. } => 0.01,
                Variable::Thickness { .. } => 1.,
                Variable::Aperture { .. } => 1.,
                Variable::Asphere { coefficient, .. } => 10f64.powi(-5 - 2 * (*coefficient as i32)),
            };
            match (settings[i].min, settings[i].max) {
                (Some(lo), Some(hi)) => ((hi - lo) * 0.2).max(f64::MIN_POSITIVE),
                _ => value(p, v).unwrap().abs().max(floor),
            }
        })
        .collect();
    for _ in 0..options.max_iterations {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let r = &eval.residual;
        let mut j = DMatrix::zeros(r.len(), variables.len());
        // Independent derivative columns share read-only inputs. Bound the worker
        // count; join before publishing progress or returning cancellation.
        let workers = std::thread::available_parallelism()
            .map_or(1, usize::from)
            .min(variables.len());
        let columns = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|worker| {
                    let best = &best;
                    let variables = &variables;
                    let scales = &scales;
                    let settings = &settings;
                    let weights = &weights;
                    let baseline = &baseline;
                    scope.spawn(move || {
                        (worker..variables.len())
                            .step_by(workers)
                            .map(|col| {
                                let v = variables[col];
                                let x = value(best, v).unwrap();
                                let evaluate = |sign: f64| {
                                    let next = (x + sign * scales[col] * 1e-3).clamp(
                                        settings[col].min.unwrap_or(f64::NEG_INFINITY),
                                        settings[col].max.unwrap_or(f64::INFINITY),
                                    );
                                    let h = (next - x) / scales[col];
                                    if h.abs() < 1e-15 || cancel.load(Ordering::Relaxed) {
                                        return None;
                                    }
                                    let mut q = best.clone();
                                    set(&mut q, v, next);
                                    crate::restoration::restore(
                                        &mut q,
                                        &options.variables,
                                        constraints,
                                    );
                                    residuals(
                                        &q,
                                        constraints,
                                        weights,
                                        options.quality.as_ref(),
                                        baseline,
                                        cancel,
                                    )
                                    .ok()
                                    .filter(|e| e.residual.len() == r.len())
                                    .map(|e| (e.residual, h))
                                };
                                let derivative = match (evaluate(1.), evaluate(-1.)) {
                                    (Some((a, ha)), Some((b, hb))) => Some((a - b) / (ha - hb)),
                                    (Some((a, h)), None) | (None, Some((a, h))) => {
                                        Some((a - r) / h)
                                    }
                                    _ => None,
                                };
                                (col, derivative)
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().map_err(|_| "Derivative worker failed"))
                .collect::<Result<Vec<_>, _>>()
        })?;
        for (col, derivative) in columns.into_iter().flatten() {
            if let Some(d) = derivative {
                j.set_column(col, &d);
            }
        }
        let jt = j.transpose();
        let lhs = &jt * &j + DMatrix::identity(variables.len(), variables.len()) * damping;
        let Some(step) = lhs.lu().solve(&(-&jt * r)) else {
            stop_reason = "线性方程无法求解".into();
            break;
        };
        let mut step_accepted = false;
        let mut movement: f64 = 0.;
        // Retry the same direction with a shorter step before increasing damping.
        // Invalid intersections never contribute zero residuals.
        for fraction in [1., 0.5, 0.25, 0.125, 0.0625] {
            if cancel.load(Ordering::Relaxed) {
                return Err("Cancelled".into());
            }
            let mut trial = best.clone();
            movement = 0.;
            for (i, v) in variables.iter().enumerate() {
                let old = value(&best, v).unwrap();
                let next = (old + fraction * step[i].clamp(-1., 1.) * scales[i]).clamp(
                    settings[i].min.unwrap_or(f64::NEG_INFINITY),
                    settings[i].max.unwrap_or(f64::INFINITY),
                );
                movement = movement.max((next - old).abs() / scales[i]);
                set(&mut trial, v, next);
            }
            crate::restoration::restore(&mut trial, &options.variables, constraints);
            if let Ok(rr) = residuals(
                &trial,
                constraints,
                &weights,
                options.quality.as_ref(),
                &baseline,
                cancel,
            ) {
                let old_violation = crate::restoration::violation(&best, constraints);
                let new_violation = crate::restoration::violation(&trial, constraints);
                if new_violation < old_violation - 1e-9
                    || (new_violation <= old_violation + 1e-9
                        && rr.residual.norm_squared() < r.norm_squared())
                {
                    best = trial;
                    eval = rr;
                    step_accepted = true;
                    damping = (damping / 3.).max(1e-8);
                    break;
                }
            }
        }
        if !step_accepted {
            damping *= 10.;
        }
        iterations += 1;
        if step_accepted && crate::restoration::violation(&best, constraints) == 0. {
            if let Ok(dense) = analyze(&best, 33, cancel) {
                if final_checks(&best, options, constraints, &before_analysis, &dense)
                    .iter()
                    .all(|c| c.passed)
                    && feasible
                        .as_ref()
                        .is_none_or(|(_, a)| balanced_rms(&dense) < balanced_rms(a))
                {
                    feasible = Some((best.clone(), dense));
                }
            }
        }
        record(iterations, &eval, &best, step_accepted);
        if damping > 1e10 {
            stop_reason = "连续试步未改善目标或几何无效，阻尼达到上限".into();
            break;
        }
        if movement < 1e-7 {
            stop_reason = "变量步长收敛或已到达上下限".into();
            break;
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".into());
    }
    let mut after_analysis = analyze(&best, 33, cancel)?;
    if let Some((saved, analysis)) = feasible {
        if !final_checks(
            &best,
            options,
            constraints,
            &before_analysis,
            &after_analysis,
        )
        .iter()
        .all(|c| c.passed)
            || balanced_rms(&analysis) < balanced_rms(&after_analysis)
        {
            best = saved;
            after_analysis = analysis;
            stop_reason.push_str("；保留已通过完整验收的最佳候选");
        }
    }
    let after = balanced_rms(&after_analysis).ok_or("No valid rays")?;
    let checks = final_checks(
        &best,
        options,
        constraints,
        &before_analysis,
        &after_analysis,
    );
    let accepted = checks.iter().all(|c| c.passed);
    Ok(Candidate {
        project: best,
        before_rms_um: before,
        after_rms_um: after,
        iterations,
        accepted,
        checks,
        sampling: 33,
        report: Some(OptimizationReport {
            focus_condition: "one_common_flat_sensor_per_design; image distance moves only if selected as variable".into(),
            before_coverage: before_analysis.coverage.clone(),
            after_coverage: after_analysis.coverage.clone(),
            stages: vec![],
            preset: options.preset,
            field_weights: weights.clone(),
            before_weighted_rms_um: weighted_rms(&before_analysis, &weights),
            after_weighted_rms_um: weighted_rms(&after_analysis, &weights),
            before_fields_rms_um: before_analysis
                .fields
                .iter()
                .map(|f| f.polychromatic_rms_um)
                .collect(),
            after_fields_rms_um: after_analysis
                .fields
                .iter()
                .map(|f| f.polychromatic_rms_um)
                .collect(),
            variables: options.variables.clone(),
            history,
            stop_reason,
        }),
        note: if accepted {
            "约束与 33×33 采样通光检查通过；局部阻尼最小二乘结果，仍需手动采纳。"
        } else {
            "未满足全部约束，禁止采纳。可保存为待改进版本，或调整变量与约束重新优化。"
        }
        .into(),
    })
}

fn final_checks(
    best: &Project,
    options: &OptimizationOptions,
    constraints: &Constraints,
    before_analysis: &Analysis,
    after_analysis: &Analysis,
) -> Vec<ConstraintCheck> {
    let mut checks = constraint_checks(best, constraints);
    if let Some(g) = &options.quality {
        checks.extend(crate::quality::checks(after_analysis, before_analysis, g));
    }
    let allow_clipping = options
        .quality
        .as_ref()
        .and_then(|g| g.min_geometric_fraction)
        .is_some_and(|f| f < 1.);
    let failed = if allow_clipping {
        after_analysis
            .spots
            .iter()
            .flat_map(|s| &s.failures)
            .filter(|f| f.status != RayStatus::Blocked)
            .map(|f| f.count)
            .sum::<usize>()
    } else {
        after_analysis.spots.iter().map(|s| s.failed).sum::<usize>()
    };
    checks.push(ConstraintCheck {
        name: if allow_clipping {"numerical_failed_rays"}else{"failed_rays"}.into(),
        surface: None,
        actual: Some(failed as f64),
        limit: 0.,
        relation: "max".into(),
        passed: failed == 0,
        reason: Some(
            if allow_clipping {"全部配置视场与波长，33×33 瞳采样；此行只检查非遮挡失败，物理遮挡另按逐视场采样比例约束检查。"}else{"全部配置视场与波长，33×33 瞳采样；失败光线包括遮挡、未命中、全反射与数值失败。"}.into(),
        ),
    });
    for setting in &options.variables {
        let actual = value(best, &setting.variable).unwrap();
        let (name, surface) = match setting.variable {
            Variable::Curvature { surface } => ("bound_curvature".to_string(), surface),
            Variable::Thickness { surface } => ("bound_thickness".to_string(), surface),
            Variable::Aperture { surface } => ("bound_aperture".to_string(), surface),
            Variable::Asphere {
                surface,
                coefficient,
            } => (format!("bound_a{}", 4 + 2 * coefficient), surface),
        };
        for (limit, relation, passed) in [
            (setting.min, "min", setting.min.is_none_or(|v| actual >= v)),
            (setting.max, "max", setting.max.is_none_or(|v| actual <= v)),
        ] {
            if let Some(limit) = limit {
                checks.push(ConstraintCheck {
                    name: name.clone(),
                    surface: Some(surface),
                    actual: Some(actual),
                    limit,
                    relation: relation.into(),
                    passed,
                    reason: None,
                });
            }
        }
    }
    checks
}

fn checked_analysis(
    p: &Project,
    guards: Option<&crate::quality::QualityGuards>,
    cancel: &AtomicBool,
) -> Result<Analysis, String> {
    let a = analyze(p, 33, cancel)?;
    let allow_clipping = guards
        .and_then(|g| g.min_geometric_fraction)
        .is_some_and(|f| f < 1.);
    if a.spots.iter().any(|s| {
        s.failed > 0
            && (!allow_clipping || s.failures.iter().any(|f| f.status != RayStatus::Blocked))
    }) {
        return Err(ray_failure_error(&a));
    }
    Ok(a)
}
fn staged_optimize(
    p: &Project,
    options: &OptimizationOptions,
    constraints: &Constraints,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(&OptimizationProgress),
) -> Result<Candidate, String> {
    let mut local = options.clone();
    local.staged = false;
    let mut seed = p.clone();
    let mut reports = Vec::new();
    let mut final_candidate = None;
    for scale in [0.5, 0.75, 1.] {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let mut original = p.clone();
        original.fields.iter_mut().for_each(|f| *f *= scale);
        original.field_spec = match &p.field_spec {
            Some(crate::fields::FieldSpec::Sensor) => {
                Some(crate::fields::FieldSpec::ImageHeights {
                    heights_mm: std::iter::once(0.)
                        .chain(crate::fields::sensor_heights(p).iter().map(|h| h * scale))
                        .collect(),
                })
            }
            Some(crate::fields::FieldSpec::ImageHeights { heights_mm }) => {
                Some(crate::fields::FieldSpec::ImageHeights {
                    heights_mm: heights_mm.iter().map(|h| h * scale).collect(),
                })
            }
            None => None,
        };
        seed.fields = original.fields.clone();
        seed.field_spec = original.field_spec.clone();
        if options.refocus_each_stage {
            original = autofocus(&original, cancel)?.project;
            seed = autofocus(&seed, cancel)?.project;
        }
        let before = checked_analysis(&original, options.quality.as_ref(), cancel)?;
        let mut candidate = optimize_configured(&seed, &local, constraints, cancel, progress)?;
        if options.refocus_each_stage {
            candidate.project = autofocus(&candidate.project, cancel)?.project;
        }
        let after = checked_analysis(&candidate.project, options.quality.as_ref(), cancel)?;
        // Rebuild every check against the FINAL refocused prescription. Old
        // checks may contain obsolete image gaps, quality values or bounds.
        candidate.checks = final_checks(&candidate.project, options, constraints, &before, &after);
        candidate.accepted = candidate.checks.iter().all(|c| c.passed);
        reports.push(StageReport {
            field_scale: scale,
            original_fields_rms_um: before
                .fields
                .iter()
                .map(|f| f.polychromatic_rms_um)
                .collect(),
            candidate_fields_rms_um: after
                .fields
                .iter()
                .map(|f| f.polychromatic_rms_um)
                .collect(),
            accepted: candidate.accepted,
            original_coverage: before.coverage.clone(),
            candidate_coverage: after.coverage.clone(),
        });
        candidate.before_rms_um = balanced_rms(&before).ok_or("No original rays")?;
        candidate.after_rms_um = balanced_rms(&after).ok_or("No candidate rays")?;
        if let Some(report) = candidate.report.as_mut() {
            report.focus_condition=if options.refocus_each_stage {"each stage: original and candidate independently refocused; common flat plane across fields"}else{"common image plane per design; no automatic refocus between stages"}.into();
            report.before_coverage = before.coverage.clone();
            report.after_coverage = after.coverage.clone();
            report.before_fields_rms_um = before
                .fields
                .iter()
                .map(|f| f.polychromatic_rms_um)
                .collect();
            report.after_fields_rms_um = after
                .fields
                .iter()
                .map(|f| f.polychromatic_rms_um)
                .collect();
            report.before_weighted_rms_um = weighted_rms(&before, &report.field_weights);
            report.after_weighted_rms_um = weighted_rms(&after, &report.field_weights);
            report.stages = reports.clone();
        }
        if !candidate.accepted {
            // An incomplete expansion is never a candidate for the full field.
            candidate.note=format!("视场扩大到 {:.0}% 时未通过原设计保护或约束；禁止采纳。报告保留已完成阶段，未声明全视场验证。",scale*100.);
            return Ok(candidate);
        }
        seed = candidate.project.clone();
        final_candidate = Some(candidate);
    }
    let mut candidate = final_candidate.unwrap();
    candidate.project.field_spec = p.field_spec.clone();
    candidate.project.fields = p.fields.clone();
    Ok(candidate)
}
pub(crate) fn balanced_rms(a: &Analysis) -> Option<f64> {
    a.fields
        .iter()
        .map(|f| f.polychromatic_rms_um)
        .collect::<Option<Vec<_>>>()
        .map(|v| (v.iter().map(|r| r * r).sum::<f64>() / v.len() as f64).sqrt())
}
