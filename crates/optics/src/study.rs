//! Multi-configuration studies. All evaluations use the existing motion/trace kernels.
use crate::{motion::*, *};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Configuration {
    pub id: String,
    pub name: String,
    pub positions_mm: Vec<f64>,
    pub object_distance: Option<f64>,
    pub target_efl_mm: Option<f64>,
    pub target_f_number: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StudyVariable {
    Curvature {
        surface: usize,
        min: f64,
        max: f64,
    },
    Thickness {
        surface: usize,
        min: f64,
        max: f64,
    },
    Position {
        configuration: usize,
        group: usize,
        min: f64,
        max: f64,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StudyOptions {
    pub mode: String,
    pub motion: MotionOptions,
    pub configurations: Vec<Configuration>,
    pub steps: usize,
    pub max_points: usize,
    pub adaptive: bool,
    pub refine_rms_um: f64,
    pub driver_group: usize,
    pub compensator_group: usize,
    pub near_mm: f64,
    pub far_mm: f64,
    pub include_infinity: bool,
    pub verification_sampling: usize,
    pub max_degradation_percent: f64,
    pub efl_tolerance_percent: Option<f64>,
    pub min_glass_mm: f64,
    pub require_sensor_coverage: bool,
    pub variables: Vec<StudyVariable>,
    pub iterations: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StudyPoint {
    pub coordinate: f64,
    pub configuration: Configuration,
    pub accepted: bool,
    pub diagnostic: String,
    pub rms_um: Option<f64>,
    pub fields_rms_um: Vec<Option<f64>>,
    pub efl_mm: Option<f64>,
    pub f_number: Option<f64>,
    pub distortion_percent: Option<f64>,
    pub lateral_color_um: Option<f64>,
    pub geometric_fraction: Option<f64>,
    pub min_air_mm: Option<f64>,
    pub min_glass_mm: Option<f64>,
    pub field_angles_rad: Vec<f64>,
    pub coverage: Option<fields::Coverage>,
    pub failed_rays: usize,
}
#[derive(Debug, Serialize)]
pub struct StudyResult {
    pub mode: String,
    pub reference: Project,
    pub configurations: Vec<Configuration>,
    pub before: Vec<StudyPoint>,
    pub points: Vec<StudyPoint>,
    pub accepted: bool,
    pub verification_sampling: usize,
    pub evaluations: usize,
    pub sensor_reference_mm: f64,
    pub note: String,
}
fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Cancelled".into())
    } else {
        Ok(())
    }
}
fn options(o: &StudyOptions, c: &Configuration, sampling: usize, solve: bool) -> MotionOptions {
    MotionOptions {
        positions_mm: c.positions_mm.clone(),
        object_distance: c.object_distance,
        target_f_number: c.target_f_number,
        sampling,
        solve_group: solve.then_some(o.compensator_group),
        ..o.motion.clone()
    }
}
fn validate(p: &Project, o: &StudyOptions) -> Result<(), String> {
    if !["scan", "trajectory", "focus", "optimize"].contains(&o.mode.as_str())
        || o.configurations.is_empty()
        || o.configurations.len() > 30
        || !(2..=21).contains(&o.steps)
        || !(2..=101).contains(&o.max_points)
        || !(17..=33).contains(&o.verification_sampling)
        || o.verification_sampling.is_multiple_of(2)
        || !(7..=33).contains(&o.motion.sampling)
        || o.motion.sampling.is_multiple_of(2)
        || o.verification_sampling < o.motion.sampling
        || ![
            o.near_mm,
            o.far_mm,
            o.refine_rms_um,
            o.max_degradation_percent,
            o.min_glass_mm,
        ]
        .iter()
        .all(|v| v.is_finite())
        || o.near_mm <= 1.
        || o.far_mm < o.near_mm
        || o.refine_rms_um <= 0.
        || o.min_glass_mm < 0.
        || o.max_degradation_percent < 0.
        || o.efl_tolerance_percent
            .is_some_and(|v| !v.is_finite() || v <= 0.)
        || o.iterations == 0
        || o.iterations > 30
        || o.variables.len() > 12
        || (o.mode == "optimize" && o.variables.is_empty())
        || (o.mode == "focus" && o.steps + usize::from(o.include_infinity) > o.max_points)
    {
        return Err("工作台配置、采样或优化范围无效".into());
    }
    for (i, c) in o.configurations.iter().enumerate() {
        if c.id.is_empty()
            || c.name.is_empty()
            || c.name.len() > 480
            || o.configurations[..i].iter().any(|v| v.id == c.id)
            || c.positions_mm.len() != o.motion.groups.len()
            || c.positions_mm.iter().any(|v| !v.is_finite())
            || c.target_efl_mm.is_some_and(|v| !v.is_finite() || v <= 0.)
        {
            return Err("命名配置无效或重复".into());
        }
    }
    if o.mode == "trajectory" || o.mode == "focus" {
        let g = o
            .motion
            .groups
            .get(o.compensator_group)
            .ok_or("请选择补偿 / 对焦镜组")?;
        if g.locked || g.min_mm >= g.max_mm {
            return Err("补偿 / 对焦镜组被锁定或没有行程".into());
        }
    }
    if o.mode == "scan" || o.mode == "trajectory" {
        if o.configurations.len() < 2
            || (o.configurations.len() - 1) * (o.steps - 1) + 1 > o.max_points
        {
            return Err("扫描至少需要两个配置；请增大点数上限或减少每段位置数".into());
        }
        if o.mode == "trajectory"
            && (o.driver_group >= o.motion.groups.len()
                || o.driver_group == o.compensator_group
                || o.motion.groups[o.driver_group].locked)
        {
            return Err("变倍组与补偿组须为不同且未锁定的镜组".into());
        }
        for pair in o.configurations.windows(2) {
            if pair[0].object_distance != pair[1].object_distance
                || pair[0].target_f_number != pair[1].target_f_number
            {
                return Err("同一变焦轨迹需要相同参考物距与光圈规则".into());
            }
            if o.mode == "trajectory"
                && (0..o.motion.groups.len()).any(|g| {
                    g != o.driver_group
                        && g != o.compensator_group
                        && pair[0].positions_mm[g] != pair[1].positions_mm[g]
                })
            {
                return Err("自动补偿时只允许变倍组和补偿组运动；其他组需保持相同位置".into());
            }
        }
    }
    for (i, v) in o.variables.iter().enumerate() {
        let (lo, hi) = bounds(v);
        let value = variable_value(p, &o.configurations, v).ok_or("联合优化变量索引无效")?;
        if !lo.is_finite()
            || !hi.is_finite()
            || lo >= hi
            || value < lo
            || value > hi
            || o.variables[..i]
                .iter()
                .any(|other| variable_key(other) == variable_key(v))
        {
            return Err("优化变量边界无效、重复或不包含当前值".into());
        }
        match *v {
            StudyVariable::Thickness { surface, min, .. }
                if surface + 1 == p.surfaces.len() || min < 0. =>
            {
                return Err("固定传感器时不可优化最后像距，厚度下限不可为负".into())
            }
            StudyVariable::Position {
                group, min, max, ..
            } => {
                let g = o.motion.groups.get(group).ok_or("镜组变量索引无效")?;
                if g.locked || min < g.min_mm || max > g.max_mm {
                    return Err("位置变量超出镜组行程或镜组被锁定".into());
                }
            }
            _ => {}
        }
    }
    Ok(())
}
fn evaluate(
    p: &Project,
    o: &StudyOptions,
    c: &Configuration,
    coordinate: f64,
    sampling: usize,
    solve: bool,
    cancel: &AtomicBool,
) -> Result<StudyPoint, String> {
    cancelled(cancel)?;
    let mut point = StudyPoint {
        coordinate,
        configuration: c.clone(),
        accepted: false,
        diagnostic: String::new(),
        rms_um: None,
        fields_rms_um: vec![],
        efl_mm: None,
        f_number: None,
        distortion_percent: None,
        lateral_color_um: None,
        geometric_fraction: None,
        min_air_mm: None,
        min_glass_mm: None,
        field_angles_rad: vec![],
        coverage: None,
        failed_rays: 0,
    };
    let opt = options(o, c, sampling, solve);
    let result = motion::run(p, &opt, cancel);
    cancelled(cancel)?;
    match result {
        Err(e) => point.diagnostic = e,
        Ok(r) => {
            point.configuration.positions_mm = r.positions_mm;
            point.accepted = r.accepted;
            point.diagnostic = r.diagnostic;
            point.rms_um = r.score_um;
            point.fields_rms_um = r
                .analysis
                .fields
                .iter()
                .map(|f| f.polychromatic_rms_um)
                .collect();
            point.field_angles_rad = r.analysis.fields.iter().map(|f| f.field).collect();
            point.efl_mm = r.analysis.paraxial.efl;
            point.f_number = r.analysis.paraxial.f_number;
            point.distortion_percent = r
                .analysis
                .spots
                .iter()
                .filter_map(|s| s.distortion_percent)
                .map(f64::abs)
                .reduce(f64::max);
            point.lateral_color_um = r
                .analysis
                .fields
                .iter()
                .filter_map(|s| s.lateral_color_um)
                .reduce(f64::max);
            point.failed_rays = r.analysis.spots.iter().map(|s| s.failed).sum();
            let sampled: usize = r.analysis.spots.iter().map(|s| s.sampled).sum();
            point.geometric_fraction =
                (sampled > 0).then(|| 1. - point.failed_rays as f64 / sampled as f64);
            point.coverage = r.analysis.coverage;
            if o.require_sensor_coverage
                && point.coverage.as_ref().is_none_or(|c| {
                    c.checkpoints.is_empty()
                        || c.checkpoints.iter().any(|p| p.status != "sampled_clear")
                })
            {
                point.accepted = false;
                point.diagnostic.push_str("；传感器检查点未全部验证通光");
            }
            let geometry = geometry::inspect(&r.project);
            point.min_air_mm = geometry
                .spans
                .iter()
                .filter(|s| !s.glass)
                .filter_map(|s| s.sampled_min_mm)
                .chain(std::iter::once(
                    r.project.surfaces.last().unwrap().thickness,
                ))
                .reduce(f64::min);
            point.min_glass_mm = geometry
                .spans
                .iter()
                .filter(|s| s.glass)
                .filter_map(|s| s.sampled_min_mm)
                .reduce(f64::min);
            if geometry
                .spans
                .iter()
                .any(|s| s.glass && !geometry::clearance_at_least(&r.project, s, o.min_glass_mm))
            {
                point.accepted = false;
                point.diagnostic.push_str("；玻璃最薄实体未达到下限");
            }
            if point.fields_rms_um.is_empty()
                || point
                    .fields_rms_um
                    .iter()
                    .any(|v| v.is_none_or(|r| r > o.motion.max_rms_um))
            {
                point.accepted = false;
                point.diagnostic.push_str("；至少一个视场未达到 RMS 上限");
            }
            if let (Some(t), Some(tol)) = (c.target_efl_mm, o.efl_tolerance_percent) {
                if point.efl_mm.is_none_or(|f| (f - t).abs() / t * 100. > tol) {
                    point.accepted = false;
                    point.diagnostic.push_str("；焦距偏差超过目标容差");
                }
            }
        }
    }
    Ok(point)
}
fn interpolate(a: &Configuration, b: &Configuration, t: f64) -> Configuration {
    Configuration {
        id: format!("{}-{t:.8}", a.id),
        name: format!("{} → {} {:.1}%", a.name, b.name, 100. * t)
            .chars()
            .take(110)
            .collect(),
        positions_mm: a
            .positions_mm
            .iter()
            .zip(&b.positions_mm)
            .map(|(a, b)| a + (b - a) * t)
            .collect(),
        target_efl_mm: a
            .target_efl_mm
            .zip(b.target_efl_mm)
            .map(|(a, b)| a + (b - a) * t),
        ..a.clone()
    }
}
fn progress_event(
    progress: &mut dyn FnMut(&OptimizationProgress),
    i: usize,
    n: usize,
    point: &StudyPoint,
    p: &Project,
) {
    progress(&OptimizationProgress {
        iteration: i,
        max_iterations: n,
        merit: point.rms_um.unwrap_or(0.),
        weighted_rms_um: point.rms_um.unwrap_or(0.),
        efl_mm: point.efl_mm,
        total_length_mm: p.image_z(),
        step_accepted: point.accepted,
    });
}
fn bounds(v: &StudyVariable) -> (f64, f64) {
    match *v {
        StudyVariable::Curvature { min, max, .. }
        | StudyVariable::Thickness { min, max, .. }
        | StudyVariable::Position { min, max, .. } => (min, max),
    }
}
fn variable_key(v: &StudyVariable) -> String {
    match v {
        StudyVariable::Curvature { surface, .. } => format!("c{surface}"),
        StudyVariable::Thickness { surface, .. } => format!("t{surface}"),
        StudyVariable::Position {
            configuration,
            group,
            ..
        } => format!("p{configuration}:{group}"),
    }
}
fn variable_value(p: &Project, c: &[Configuration], v: &StudyVariable) -> Option<f64> {
    match *v {
        StudyVariable::Curvature { surface, .. } => p.surfaces.get(surface).map(|s| s.curvature),
        StudyVariable::Thickness { surface, .. } => p.surfaces.get(surface).map(|s| s.thickness),
        StudyVariable::Position {
            configuration,
            group,
            ..
        } => c.get(configuration)?.positions_mm.get(group).copied(),
    }
}
fn change(p: &mut Project, c: &mut [Configuration], v: &StudyVariable, x: f64) {
    match *v {
        StudyVariable::Curvature { surface, .. } => p.surfaces[surface].curvature = x,
        StudyVariable::Thickness { surface, .. } => {
            let delta = x - p.surfaces[surface].thickness;
            p.surfaces[surface].thickness = x;
            p.surfaces.last_mut().unwrap().thickness -= delta;
        }
        StudyVariable::Position {
            configuration,
            group,
            ..
        } => c[configuration].positions_mm[group] = x,
    }
}
fn all(
    p: &Project,
    o: &StudyOptions,
    c: &[Configuration],
    sampling: usize,
    cancel: &AtomicBool,
) -> Result<Vec<StudyPoint>, String> {
    c.iter()
        .enumerate()
        .map(|(i, c)| evaluate(p, o, c, i as f64, sampling, false, cancel))
        .collect()
}
fn rms(points: &[StudyPoint]) -> Option<f64> {
    let values: Option<Vec<f64>> = points
        .iter()
        .flat_map(|p| p.fields_rms_um.iter().copied())
        .collect();
    if points
        .iter()
        .any(|p| p.rms_um.is_none() || p.failed_rays > 0)
    {
        return None;
    }
    values
        .filter(|v| !v.is_empty())
        .map(|v| (v.iter().map(|r| r * r).sum::<f64>() / v.len() as f64).sqrt())
}
fn protect(points: &mut [StudyPoint], baseline: &[StudyPoint], degradation: f64) {
    for (a, b) in points.iter_mut().zip(baseline) {
        if a.fields_rms_um.len() != b.fields_rms_um.len()
            || a.fields_rms_um
                .iter()
                .zip(&b.fields_rms_um)
                .any(|(a, b)| match (a, b) {
                    (Some(a), Some(b)) => *a > b * (1. + degradation / 100.) + 1e-9,
                    _ => true,
                })
        {
            a.accepted = false;
            a.diagnostic.push_str("；逐视场退化超过允许比例");
        }
    }
}
pub fn run(
    p: &Project,
    o: &StudyOptions,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(&OptimizationProgress),
) -> Result<StudyResult, String> {
    validate(p, o)?;
    cancelled(cancel)?;
    let mut reference = p.clone();
    let mut configs = o.configurations.clone();
    let mut before = vec![];
    let mut points = vec![];
    let mut evaluations = 0;
    if o.mode == "optimize" {
        let baseline = all(p, o, &configs, o.motion.sampling, cancel)?;
        let mut best =
            rms(&baseline).ok_or("原设计存在失败光线或无有效视场，无法建立逐视场保护基线")?;
        let mut step_scale = 0.2;
        for iteration in 0..o.iterations {
            let mut improved = false;
            for variable in &o.variables {
                let (lo, hi) = bounds(variable);
                let start = variable_value(&reference, &configs, variable).unwrap();
                for direction in [-1., 1.] {
                    cancelled(cancel)?;
                    let mut q = reference.clone();
                    let mut c = configs.clone();
                    change(
                        &mut q,
                        &mut c,
                        variable,
                        (start + direction * (hi - lo) * step_scale).clamp(lo, hi),
                    );
                    let mut tested = all(&q, o, &c, o.motion.sampling, cancel)?;
                    evaluations += tested.len();
                    protect(&mut tested, &baseline, o.max_degradation_percent);
                    if tested.iter().all(|p| p.accepted)
                        && rms(&tested).is_some_and(|s| s < best - 1e-9)
                    {
                        best = rms(&tested).unwrap();
                        reference = q;
                        configs = c;
                        improved = true;
                    }
                }
            }
            progress(&OptimizationProgress {
                iteration: iteration + 1,
                max_iterations: o.iterations,
                merit: best,
                weighted_rms_um: best,
                efl_mm: None,
                total_length_mm: reference.image_z(),
                step_accepted: improved,
            });
            if !improved {
                step_scale *= 0.5;
            }
        }
        before = all(p, o, &o.configurations, o.verification_sampling, cancel)?;
        points = all(&reference, o, &configs, o.verification_sampling, cancel)?;
        evaluations += before.len() + points.len();
        protect(&mut points, &before, o.max_degradation_percent);
        if rms(&points)
            .zip(rms(&before))
            .is_none_or(|(a, b)| a >= b - 1e-9)
        {
            for point in &mut points {
                point.accepted = false;
                point.diagnostic.push_str("；高采样复核未改善总体 RMS");
            }
        }
    } else {
        let mut jobs: Vec<(f64, Configuration)> = vec![];
        if o.mode == "focus" {
            if o.include_infinity {
                let mut c = configs[0].clone();
                c.object_distance = None;
                c.name = "无限远".into();
                jobs.push((0., c));
            }
            for i in 0..o.steps {
                let t = i as f64 / (o.steps - 1) as f64;
                let distance = o.far_mm * (o.near_mm / o.far_mm).powf(t);
                let mut c = configs[0].clone();
                c.object_distance = Some(distance);
                c.name = format!("物距 {distance:.2} mm");
                jobs.push((1. / distance, c));
            }
        } else {
            for (segment, pair) in configs.windows(2).enumerate() {
                for i in 0..o.steps {
                    if segment > 0 && i == 0 {
                        continue;
                    }
                    let t = i as f64 / (o.steps - 1) as f64;
                    jobs.push((segment as f64 + t, interpolate(&pair[0], &pair[1], t)));
                }
            }
        }
        for (coordinate, c) in jobs {
            let mut point = evaluate(
                p,
                o,
                &c,
                coordinate,
                if o.mode == "scan" {
                    o.verification_sampling
                } else {
                    o.motion.sampling
                },
                o.mode != "scan",
                cancel,
            )?;
            if o.mode != "scan" && point.rms_um.is_some() {
                let boundary = !point.accepted && point.diagnostic.contains("行程边界");
                point = evaluate(
                    p,
                    o,
                    &point.configuration,
                    coordinate,
                    o.verification_sampling,
                    false,
                    cancel,
                )?;
                if boundary {
                    point.accepted = false;
                    point.diagnostic.push_str("；求解最优位置在行程边界");
                }
            }
            evaluations += 1;
            progress_event(progress, evaluations, o.max_points, &point, p);
            points.push(point);
        }
        // Refine uncertain or rapidly changing intervals, never hide failed endpoints.
        if o.adaptive && o.mode != "focus" {
            for _ in 0..3 {
                let mut extra = vec![];
                for pair in points.windows(2) {
                    if points.len() + extra.len() >= o.max_points {
                        break;
                    }
                    if !pair[0].accepted
                        || !pair[1].accepted
                        || pair[0]
                            .rms_um
                            .zip(pair[1].rms_um)
                            .is_some_and(|(a, b)| (a - b).abs() > o.refine_rms_um)
                    {
                        let x = (pair[0].coordinate + pair[1].coordinate) * 0.5;
                        let segment = (x.floor() as usize).min(configs.len() - 2);
                        let c = interpolate(
                            &configs[segment],
                            &configs[segment + 1],
                            x - segment as f64,
                        );
                        let mut point = evaluate(
                            p,
                            o,
                            &c,
                            x,
                            if o.mode == "scan" {
                                o.verification_sampling
                            } else {
                                o.motion.sampling
                            },
                            o.mode != "scan",
                            cancel,
                        )?;
                        if o.mode == "trajectory" && point.rms_um.is_some() {
                            let boundary = point.diagnostic.contains("行程边界");
                            point = evaluate(
                                p,
                                o,
                                &point.configuration,
                                x,
                                o.verification_sampling,
                                false,
                                cancel,
                            )?;
                            if boundary {
                                point.accepted = false;
                                point.diagnostic.push_str("；求解最优位置在行程边界");
                            }
                        }
                        evaluations += 1;
                        progress_event(progress, evaluations, o.max_points, &point, p);
                        extra.push(point);
                    }
                }
                if extra.is_empty() {
                    break;
                }
                points.extend(extra);
                points.sort_by(|a, b| a.coordinate.total_cmp(&b.coordinate));
            }
        }
        for (i, point) in points.iter_mut().enumerate() {
            point.configuration.id = format!("study-{i}");
        }
        configs = points.iter().map(|p| p.configuration.clone()).collect();
    }
    cancelled(cancel)?;
    Ok(StudyResult {mode:o.mode.clone(),sensor_reference_mm:p.image_z(),reference,configurations:configs,
        accepted:!points.is_empty() && points.iter().all(|p|p.accepted),before,points,verification_sampling:o.verification_sampling,evaluations,
        note:"固定传感器；逐视场 RMS / 通光 / 几何约束；已验证离散位置，不证明连续齐焦或全局最优；几何通光比例不是精确照度，PSF/MTF 未在本任务计算。".into()})
}
