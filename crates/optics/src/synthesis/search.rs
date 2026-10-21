use super::*;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Exploration {
    #[default]
    Fixed,
    Expanded,
}
pub fn default_attempts() -> usize {
    36
}
pub fn default_center() -> f64 {
    0.5
}
pub fn default_edge() -> f64 {
    0.1
}
pub(super) const PAIRS: [(&str, &str); 3] = [
    ("SCHOTT N-BK7", "SCHOTT N-F2"),
    ("SCHOTT N-BAK4", "SCHOTT N-SF5"),
    ("SCHOTT N-SK16", "SCHOTT N-SF10"),
];
pub(super) fn catalog_material(name: &str) -> Result<Material, String> {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../../../../src/data/glass-catalog.json"))
            .map_err(|e| e.to_string())?;
    let item = data["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == name)
        .ok_or("Missing catalog material")?;
    serde_json::from_value(item.clone()).map_err(|e| e.to_string())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchUnit {
    pub structure: String,
    pub pair: usize,
    pub start: usize,
    pub mutation: usize,
    pub glass: usize,
}
impl SearchUnit {
    pub fn label(&self) -> String {
        format!(
            "{} / {}",
            ["原结构", "移动光阑", "后置校正片", "前置负透镜"][self.mutation],
            if self.glass == 0 {
                "原玻璃".into()
            } else {
                format!("第 {} 片独立换玻璃", (self.glass - 1) / 2 + 1)
            }
        )
    }
}
pub(super) fn mutate(p: &mut Project, u: &SearchUnit) -> Result<(), String> {
    if u.glass > 0 {
        let element = (u.glass - 1) / 2;
        let index = p
            .surfaces
            .iter()
            .enumerate()
            .filter(|(_, s)| s.material != 0)
            .nth(element)
            .map(|(i, _)| i)
            .ok_or("Glass element does not exist")?;
        let old = p.surfaces[index].material;
        let pair = PAIRS[((u.glass - 1) % 2 + u.pair + 1) % 3];
        let material = catalog_material(if old == 1 { pair.0 } else { pair.1 })?;
        // New snapshot index means other elements sharing the original glass stay unchanged.
        p.surfaces[index].material = p.materials.len();
        p.materials.push(material);
    }
    if u.mutation == 1 {
        let before = p
            .stop
            .checked_sub(1)
            .filter(|&i| p.surfaces[i].material == 0)
            .ok_or("Stop has no preceding air space")?;
        let sum = p.surfaces[before].thickness + p.surfaces[p.stop].thickness;
        p.surfaces[before].thickness = sum * 0.7;
        p.surfaces[p.stop].thickness = sum * 0.3;
    } else if u.mutation >= 2 {
        let mut front = p.surfaces[0].clone();
        front.name = "结构探索 · 校正片前面".into();
        front.curvature = if u.mutation == 3 {
            -1. / 120.
        } else {
            1. / 180.
        };
        front.thickness = 2.;
        front.material = 1;
        front.mechanical = None;
        front.coating.clear();
        let mut back = front.clone();
        back.name = "结构探索 · 校正片后面".into();
        back.curvature = -front.curvature;
        back.material = 0;
        if u.mutation == 3 {
            back.thickness = 5.;
            p.surfaces.splice(0..0, [front, back]);
            p.stop += 2;
        } else {
            let last = p.surfaces.len() - 1;
            back.thickness = p.surfaces[last].thickness;
            p.surfaces[last].thickness = 5.;
            p.surfaces.extend([front, back]);
        }
    }
    Ok(())
}
pub fn plan(b: &DesignBrief) -> Result<Vec<SearchUnit>, String> {
    if b.max_attempts == 0
        || b.max_attempts > 240
        || !b.min_center_mm.is_finite()
        || !b.min_edge_mm.is_finite()
        || b.min_center_mm < 0.01
        || b.min_edge_mm < 0.
        || b.min_edge_mm > b.min_center_mm
        || b.min_center_mm > 100.
    {
        return Err("搜索预算 1–240；中心厚度 0.01–100 mm，边缘厚度不得大于中心厚度。".into());
    }
    let structures = if b.structure == "auto" {
        vec!["tessar", "planar", "double_gauss", "sonnar"]
    } else {
        vec![b.structure.as_str()]
    };
    for name in &structures {
        seed(name)?;
    }
    let mut units = vec![];
    // Round-robin by topology first, so a bounded budget does not spend everything on Tessar.
    for glass in 0..if b.element_glass { 13 } else { 1 } {
        for mutation in 0..if b.exploration == Exploration::Expanded {
            4
        } else {
            1
        } {
            for start in 0..b.starts {
                for pair in 0..if b.glass_search { 3 } else { 1 } {
                    for structure in &structures {
                        let count = seed(structure)?
                            .surfaces
                            .iter()
                            .filter(|s| s.material != 0)
                            .count();
                        if glass > count * 2 {
                            continue;
                        }
                        units.push(SearchUnit {
                            structure: (*structure).into(),
                            pair,
                            start,
                            mutation,
                            glass,
                        });
                        if units.len() >= b.max_attempts {
                            return Ok(units);
                        }
                    }
                }
            }
        }
    }
    Ok(units)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub engine_version: String,
    pub brief: DesignBrief,
    pub cursor: usize,
    pub total: usize,
    pub candidates: Vec<Candidate>,
    pub attempts: Vec<String>,
}
#[derive(Serialize)]
pub struct SearchStep {
    pub checkpoint: Checkpoint,
    pub outcome: DesignOutcome,
    pub done: bool,
}
fn rank(c: &mut [Candidate]) {
    c.sort_by(|a, b| {
        b.accepted
            .cmp(&a.accepted)
            .then(a.after_rms_um.total_cmp(&b.after_rms_um))
            .then(a.project.surfaces.len().cmp(&b.project.surfaces.len()))
    });
}

/// One complete deterministic work unit per checkpoint. Cancellation restarts
/// only the unfinished unit; there is no hidden RNG or optimizer state to restore.
pub fn step(
    b: &DesignBrief,
    saved: Option<&Checkpoint>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(&OptimizationProgress),
) -> Result<SearchStep, String> {
    let units = plan(b)?;
    let mut checkpoint = if let Some(cp) = saved {
        if cp.engine_version != "1.5.0-search1"
            || serde_json::to_value(&cp.brief).unwrap() != serde_json::to_value(b).unwrap()
            || cp.total != units.len()
            || cp.cursor > cp.total
            || cp.attempts.len() != cp.cursor
            || cp.candidates.len() > 8
        {
            return Err("搜索断点与任务或引擎不匹配，请开始新搜索。".into());
        }
        let mut cp = cp.clone();
        // Persisted acceptance flags are not trusted. Recheck every retained
        // candidate against the task before letting it back into the result pool.
        for c in &mut cp.candidates {
            verify(c, b, cancel)?;
        }
        cp
    } else {
        Checkpoint {
            engine_version: "1.5.0-search1".into(),
            brief: b.clone(),
            cursor: 0,
            total: units.len(),
            candidates: vec![],
            attempts: vec![],
        }
    };
    if checkpoint.cursor < units.len() {
        let unit = &units[checkpoint.cursor];
        match run_unit(unit, b, cancel, progress) {
            Ok(mut result) => {
                checkpoint.attempts.push(format!(
                    "{} · {}",
                    unit.label(),
                    result.attempts.join("；")
                ));
                checkpoint.candidates.append(&mut result.candidates);
            }
            Err(e) => {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                checkpoint.attempts.push(format!("{}：{}", unit.label(), e));
            }
        }
        checkpoint.cursor += 1;
        rank(&mut checkpoint.candidates);
        checkpoint.candidates.truncate(8);
        for c in &mut checkpoint.candidates {
            if let Some(r) = &mut c.report {
                r.history.clear();
            }
        }
    }
    rank(&mut checkpoint.candidates);
    let outcome=DesignOutcome{candidate:checkpoint.candidates.first().cloned(),candidates:checkpoint.candidates.clone(),attempts:checkpoint.attempts.clone(),brief:b.clone(),
        note:"确定性有界搜索；保留最佳 8 个候选。通过仅指任务规定的几何、F/d/C 与 33×33 采样验收，不代表制造或 MTF 通过。断点保存已完成尝试，未完成尝试重跑。".into()};
    Ok(SearchStep {
        done: checkpoint.cursor == checkpoint.total,
        checkpoint,
        outcome,
    })
}
fn verify(c: &mut Candidate, b: &DesignBrief, cancel: &AtomicBool) -> Result<(), String> {
    if !c.project.validate_optical().is_empty() {
        return Err("断点候选几何无效".into());
    }
    // A saved candidate must retain the final task's sampling locations and sensor.
    let expected = if b.require_sensor_coverage {
        Some(fields::FieldSpec::ImageHeights {
            heights_mm: vec![
                0.,
                b.sensor_mm[0].min(b.sensor_mm[1]) / 2.,
                b.sensor_mm[0].max(b.sensor_mm[1]) / 2.,
                b.sensor_mm[0].hypot(b.sensor_mm[1]) / 2.,
            ],
        })
    } else {
        None
    };
    if c.project.object_distance.is_some()
        || serde_json::to_value(&c.project.wavelengths).unwrap()
            != serde_json::to_value(seed("tessar")?.wavelengths).unwrap()
        || c.project.sensor != b.sensor_mm
        || serde_json::to_value(&c.project.field_spec).unwrap()
            != serde_json::to_value(expected).unwrap()
        || (!b.require_sensor_coverage
            && c.project.fields
                != vec![
                    0.,
                    b.half_field_deg.to_radians() * 0.7,
                    b.half_field_deg.to_radians(),
                ])
    {
        return Err("断点候选与目标视场不一致".into());
    }
    let constraints = Constraints {
        target_efl_mm: b.focal_length_mm,
        efl_tolerance_mm: b.focal_length_mm * 0.01,
        max_length_mm: b.max_length_mm,
        min_center_mm: b.min_center_mm,
        min_edge_mm: b.min_edge_mm,
        min_air_mm: 0.,
        design: Some(DesignTargets {
            f_number: b.f_number,
            f_tolerance: b.f_number * 0.03,
            min_bfl_mm: 0.01,
            max_front_diameter_mm: 1000.,
        }),
    };
    let a = analyze(&c.project, 33, cancel)?;
    let mut checks = constraint_checks(&c.project, &constraints);
    let g = quality::QualityGuards {
        max_worst_rms_um: Some(b.max_rms_um),
        require_sensor_coverage: b.require_sensor_coverage,
        ..Default::default()
    };
    checks.extend(quality::checks(&a, &a, &g));
    let failed = a.spots.iter().map(|s| s.failed).sum::<usize>();
    checks.push(ConstraintCheck {
        name: "failed_rays".into(),
        surface: None,
        actual: Some(failed as f64),
        limit: 0.,
        relation: "max".into(),
        passed: failed == 0,
        reason: Some("断点恢复时重新追迹".into()),
    });
    c.after_rms_um = optimize::balanced_rms(&a).ok_or("断点无有效光线")?;
    c.checks = checks;
    c.accepted = c.checks.iter().all(|v| v.passed);
    c.sampling = 33;
    Ok(())
}
pub fn run(
    b: &DesignBrief,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(&OptimizationProgress),
) -> Result<DesignOutcome, String> {
    let units = plan(b)?;
    let mut candidates = vec![];
    let mut attempts = vec![];
    for unit in &units {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        match run_unit(unit, b, cancel, progress) {
            Ok(mut r) => {
                candidates.append(&mut r.candidates);
                attempts.extend(
                    r.attempts
                        .into_iter()
                        .map(|a| format!("{} · {a}", unit.label())),
                );
            }
            Err(e) => attempts.push(format!("{}：{}", unit.label(), e)),
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".into());
    }
    rank(&mut candidates);
    Ok(DesignOutcome {
        candidate: candidates.first().cloned(),
        candidates,
        attempts,
        brief: b.clone(),
        note: "有限结构/材料搜索与约束恢复；最终按 33×33 几何采样验收，不保证任意指标可解。".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_glass_mutation_changes_only_one_element_even_when_material_is_shared() {
        let mut p = seed("sonnar").unwrap();
        let original = p.clone();
        mutate(
            &mut p,
            &SearchUnit {
                structure: "sonnar".into(),
                pair: 0,
                start: 0,
                mutation: 0,
                glass: 1,
            },
        )
        .unwrap();
        let changed: Vec<_> = p
            .surfaces
            .iter()
            .zip(&original.surfaces)
            .filter(|(a, b)| a.material != b.material)
            .collect();
        assert_eq!(changed.len(), 1);
        assert_eq!(p.materials.len(), original.materials.len() + 1);
        assert_ne!(
            p.materials[changed[0].0.material].name,
            p.materials[changed[0].1.material].name
        );
    }
    #[test]
    fn expanded_seeds_change_topology_and_preserve_stop_reference() {
        for mutation in [2, 3] {
            let mut p = seed("double_gauss").unwrap();
            let count = p.surfaces.len();
            let stop = p.stop;
            mutate(
                &mut p,
                &SearchUnit {
                    structure: "double_gauss".into(),
                    pair: 0,
                    start: 0,
                    mutation,
                    glass: 0,
                },
            )
            .unwrap();
            assert_eq!(p.surfaces.len(), count + 2);
            assert_eq!(p.stop, stop + if mutation == 3 { 2 } else { 0 });
            assert_eq!(p.surfaces[p.stop].curvature, 0.);
        }
    }
}
