//! Image-distance sweep. Report failed/clipped rays, never reward their absence.
use crate::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusCriterion {
    Center,
    Balanced,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FocusOptions {
    pub min_mm: f64,
    pub max_mm: f64,
    pub steps: usize,
    pub sampling: usize,
    pub criterion: FocusCriterion,
}
#[derive(Debug, Serialize)]
pub struct FocusPoint {
    pub coverage: Option<crate::fields::Coverage>,
    pub fields_rad: Vec<f64>,
    pub image_distance_mm: f64,
    pub fields_rms_um: Vec<Option<f64>>,
    pub score_um: Option<f64>,
    pub failed: usize,
    pub sampled: usize,
    pub diagnostic: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct FocusScan {
    pub options: FocusOptions,
    pub fields_rad: Vec<f64>,
    pub points: Vec<FocusPoint>,
    pub best_index: Option<usize>,
    pub boundary_best: bool,
}
pub fn focus_scan(
    p: &Project,
    options: &FocusOptions,
    cancel: &AtomicBool,
) -> Result<FocusScan, String> {
    if !p.validate_optical().is_empty() {
        return Err("Invalid project".into());
    }
    if !options.min_mm.is_finite()
        || !options.max_mm.is_finite()
        || options.min_mm < 0.01
        || options.max_mm > 1e5
        || options.max_mm <= options.min_mm
        || !(3..=101).contains(&options.steps)
        || !(7..=33).contains(&options.sampling)
        || options.sampling.is_multiple_of(2)
    {
        return Err(
            "Focus scan: use 0.01 <= min < max <= 100000 mm, 3..101 steps and odd sampling 7..33"
                .into(),
        );
    }
    let center = p.fields.iter().position(|f| f.abs() < 1e-12);
    if matches!(options.criterion, FocusCriterion::Center) && center.is_none() {
        return Err("Center focus requires a zero-angle field".into());
    }
    let mut q = p.clone();
    let last = q.surfaces.len() - 1;
    let mut points = Vec::new();
    for i in 0..options.steps {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let distance = options.min_mm
            + (options.max_mm - options.min_mm) * i as f64 / (options.steps - 1) as f64;
        q.surfaces[last].thickness = distance;
        let point = match analyze(&q, options.sampling, cancel) {
            Ok(a) => {
                let failed = a.spots.iter().map(|s| s.failed).sum();
                let sampled = a.spots.iter().map(|s| s.sampled).sum();
                let values: Vec<_> = a.fields.iter().map(|f| f.polychromatic_rms_um).collect();
                let score = if failed > 0 || values.iter().any(Option::is_none) {
                    None
                } else {
                    match options.criterion {
                        FocusCriterion::Center => values[center.unwrap()],
                        FocusCriterion::Balanced => Some(
                            (values.iter().map(|v| v.unwrap().powi(2)).sum::<f64>()
                                / values.len() as f64)
                                .sqrt(),
                        ),
                    }
                };
                FocusPoint {
                    coverage: a.coverage.clone(),
                    fields_rad: a.fields.iter().map(|f| f.field).collect(),
                    image_distance_mm: distance,
                    fields_rms_um: values,
                    score_um: score,
                    failed,
                    sampled,
                    diagnostic: (failed > 0).then(|| {
                        "Clipped or failed rays; excluded from best-focus selection".into()
                    }),
                }
            }
            Err(e) => FocusPoint {
                coverage: None,
                fields_rad: vec![],
                image_distance_mm: distance,
                fields_rms_um: vec![None; p.fields.len()],
                score_um: None,
                failed: 0,
                sampled: 0,
                diagnostic: Some(e),
            },
        };
        points.push(point);
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".into());
    }
    let best_index = points
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.score_um.map(|s| (i, s)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i);
    Ok(FocusScan {
        options: options.clone(),
        fields_rad: p.fields.clone(),
        boundary_best: best_index.is_some_and(|i| i == 0 || i + 1 == points.len()),
        points,
        best_index,
    })
}
