use std::sync::atomic::{AtomicBool, Ordering};
use wollaston::*;
fn limits(p: &Project) -> Constraints {
    Constraints {
        target_efl_mm: paraxial(p, 587.5618).unwrap().efl.unwrap(),
        efl_tolerance_mm: 0.5,
        max_length_mm: 100.,
        min_center_mm: 1.,
        min_edge_mm: 0.2,
        min_air_mm: 0.,
        design: None,
    }
}
fn settings(p: &Project) -> OptimizationOptions {
    OptimizationOptions {
        quality: None,
        staged: false,
        refocus_each_stage: false,
        preset: AnalysisPreset::Edge,
        max_iterations: 30,
        variables: vec![
            VariableSetting {
                variable: Variable::Curvature { surface: 1 },
                min: Some(p.surfaces[1].curvature - 0.002),
                max: Some(p.surfaces[1].curvature + 0.002),
                locked: true,
            },
            VariableSetting {
                variable: Variable::Thickness { surface: 2 },
                min: Some(46.),
                max: Some(47.),
                locked: false,
            },
        ],
    }
}
#[test]
fn bounded_multi_surface_search_preserves_locks_and_reports_every_blocker() {
    let p = examples().remove(0);
    let original = serde_json::to_value(&p).unwrap();
    let options = settings(&p);
    let mut events = vec![];
    let c = optimize_configured(
        &p,
        &options,
        &limits(&p),
        &AtomicBool::new(false),
        &mut |e| events.push(e.clone()),
    )
    .unwrap();
    assert_eq!(c.project.surfaces[1].curvature, p.surfaces[1].curvature);
    assert!((46. ..=47.).contains(&c.project.surfaces[2].thickness));
    assert_eq!(c.accepted, c.checks.iter().all(|c| c.passed));
    let report = c.report.unwrap();
    assert_eq!(events.len(), c.iterations + 1);
    assert_eq!(events.len(), report.history.len());
    assert!(!report.stop_reason.is_empty());
    assert_eq!(report.field_weights.len(), p.fields.len());
    assert!(report.history.windows(2).all(|h| h[1].merit <= h[0].merit));
    assert_eq!(serde_json::to_value(&p).unwrap(), original);
    let mut impossible = limits(&p);
    impossible.target_efl_mm = 500.;
    let rejected = optimize_configured(
        &p,
        &options,
        &impossible,
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap();
    assert!(!rejected.accepted);
    assert!(rejected
        .checks
        .iter()
        .any(|c| !c.passed && c.name == "efl_error_mm"));
}
#[test]
fn multi_face_bounds_and_asphere_are_enforced_and_unselected_values_are_fixed() {
    let p = examples().remove(0);
    let mut o = settings(&p);
    o.variables[0].locked = false;
    o.variables.push(VariableSetting {
        variable: Variable::Asphere {
            surface: 1,
            coefficient: 0,
        },
        min: Some(-1e-7),
        max: Some(1e-7),
        locked: false,
    });
    let c = optimize_configured(&p, &o, &limits(&p), &AtomicBool::new(false), &mut |_| {}).unwrap();
    for s in [0, 2] {
        assert_eq!(c.project.surfaces[s].curvature, p.surfaces[s].curvature);
    }
    assert_eq!(c.project.surfaces[1].thickness, p.surfaces[1].thickness);
    assert!(c.project.surfaces[1].asphere[0].abs() <= 1e-7);
    assert!(c
        .checks
        .iter()
        .filter(|c| c.name.starts_with("bound_"))
        .all(|c| c.passed));
}
#[test]
fn invalid_bounds_duplicates_locks_and_live_cancellation_fail_safely() {
    let p = examples().remove(0);
    let c = limits(&p);
    for mode in 0..5 {
        let mut o = settings(&p);
        match mode {
            0 => o.variables[1].min = Some(100.),
            1 => o.variables[1].max = Some(f64::INFINITY),
            2 => o.variables.push(o.variables[1].clone()),
            3 => o.variables[1].locked = true,
            _ => o.max_iterations = 101,
        }
        assert!(optimize_configured(&p, &o, &c, &AtomicBool::new(false), &mut |_| {}).is_err());
    }
    let cancel = AtomicBool::new(false);
    let mut count = 0;
    assert!(
        optimize_configured(&p, &settings(&p), &c, &cancel, &mut |_| {
            count += 1;
            cancel.store(true, Ordering::Relaxed);
        })
        .is_err()
    );
    assert_eq!(count, 1);
}
#[test]
fn preset_weights_are_normalized_symmetric_and_never_hide_a_field() {
    let fields = [0., 0.07, 0.1, -0.1];
    for preset in [
        AnalysisPreset::Center,
        AnalysisPreset::Balanced,
        AnalysisPreset::Edge,
    ] {
        let w = field_weights(&fields, preset);
        assert!((w.iter().sum::<f64>() - 1.).abs() < 1e-14);
        assert!(w.iter().all(|w| *w > 0.));
        assert_eq!(w[2], w[3]);
    }
    assert!(
        field_weights(&fields, AnalysisPreset::Center)[0]
            > field_weights(&fields, AnalysisPreset::Center)[2]
    );
    assert!(
        field_weights(&fields, AnalysisPreset::Edge)[0]
            < field_weights(&fields, AnalysisPreset::Edge)[2]
    );
}
