use std::sync::atomic::AtomicBool;
use wollaston::*;
fn flag() -> AtomicBool {
    AtomicBool::new(false)
}
fn options() -> FocusOptions {
    FocusOptions {
        min_mm: 40.,
        max_mm: 60.,
        steps: 41,
        sampling: 9,
        criterion: FocusCriterion::Balanced,
    }
}
fn constraints(p: &Project) -> Constraints {
    Constraints {
        target_efl_mm: paraxial(p, 587.5618).unwrap().efl.unwrap(),
        efl_tolerance_mm: 0.01,
        max_length_mm: 100.,
        min_center_mm: 1.,
        min_edge_mm: 0.2,
        min_air_mm: 0.,
        design: None,
    }
}

#[test]
fn scan_agrees_with_direct_analysis_and_quadratic_focus_without_mutating() {
    let p = examples().remove(0);
    let original = serde_json::to_value(&p).unwrap();
    let scan = focus_scan(&p, &options(), &flag()).unwrap();
    let best = scan.best_index.unwrap();
    assert!(!scan.boundary_best);
    for i in [0, best, scan.points.len() - 1] {
        let mut q = p.clone();
        q.surfaces.last_mut().unwrap().thickness = scan.points[i].image_distance_mm;
        let a = analyze(&q, 9, &flag()).unwrap();
        for (f, rms) in a.fields.iter().zip(&scan.points[i].fields_rms_um) {
            assert_eq!(f.polychromatic_rms_um, *rms);
        }
    }
    // For a fixed emerging bundle, RMS^2 is quadratic in image distance.
    // Recover its minimum independently from three widely separated samples.
    let (a, b, c) = (
        scan.points[0].score_um.unwrap().powi(2),
        scan.points[20].score_um.unwrap().powi(2),
        scan.points[40].score_um.unwrap().powi(2),
    );
    let exact = 50. + 5. * (a - c) / (a - 2. * b + c);
    assert!((scan.points[best].image_distance_mm - exact).abs() <= 0.250001);
    assert_eq!(serde_json::to_value(&p).unwrap(), original);
}
#[test]
fn focus_criteria_boundary_and_failure_diagnostics() {
    let mut p = examples().remove(0);
    let mut o = options();
    o.criterion = FocusCriterion::Center;
    let s = focus_scan(&p, &o, &flag()).unwrap();
    assert!(s
        .points
        .iter()
        .all(|point| point.score_um == point.fields_rms_um[0]));
    o.min_mm = 10.;
    o.max_mm = 15.;
    assert!(focus_scan(&p, &o, &flag()).unwrap().boundary_best);
    p.surfaces[1].semi_aperture = 0.001;
    let s = focus_scan(&p, &o, &flag()).unwrap();
    assert!(s.best_index.is_none());
    assert!(s
        .points
        .iter()
        .all(|p| p.failed > 0 && p.score_um.is_none() && p.diagnostic.is_some()));
}
#[test]
fn focus_rejects_invalid_options_and_cancelled_work() {
    let p = examples().remove(0);
    assert!(focus_scan(&p, &options(), &AtomicBool::new(true)).is_err());
    let mut o = options();
    o.steps = 102;
    assert!(focus_scan(&p, &o, &flag()).is_err());
    o = options();
    o.sampling = 10;
    assert!(focus_scan(&p, &o, &flag()).is_err());
    o = options();
    o.min_mm = f64::NAN;
    assert!(focus_scan(&p, &o, &flag()).is_err());
    let mut p = p;
    p.fields = vec![0.1];
    o = options();
    o.criterion = FocusCriterion::Center;
    assert!(focus_scan(&p, &o, &flag()).is_err());
}
#[test]
fn constrained_candidate_reports_dense_metrics_and_changes_only_selected_variables() {
    let p = examples().remove(0);
    let original = serde_json::to_value(&p).unwrap();
    let c = optimize_constrained(
        &p,
        &[Variable::Thickness {
            surface: p.surfaces.len() - 1,
        }],
        &constraints(&p),
        &flag(),
    )
    .unwrap();
    assert!(c.accepted);
    assert_eq!(c.sampling, 33);
    assert!(c.checks.iter().all(|c| c.passed));
    assert!(c.after_rms_um < c.before_rms_um);
    let a = analyze(&c.project, 33, &flag()).unwrap();
    let rms = (a
        .fields
        .iter()
        .map(|f| f.polychromatic_rms_um.unwrap().powi(2))
        .sum::<f64>()
        / a.fields.len() as f64)
        .sqrt();
    assert_eq!(c.after_rms_um, rms);
    let mut restored = c.project;
    restored.surfaces.last_mut().unwrap().thickness = p.surfaces.last().unwrap().thickness;
    assert_eq!(serde_json::to_value(restored).unwrap(), original);
    assert_eq!(serde_json::to_value(p).unwrap(), original);
}
#[test]
fn impossible_constraints_are_explicitly_non_adoptable() {
    let p = examples().remove(0);
    let mut limits = constraints(&p);
    limits.target_efl_mm = 200.;
    limits.max_length_mm = 1.;
    limits.min_center_mm = 20.;
    limits.min_edge_mm = 20.;
    limits.min_air_mm = 20.;
    let c =
        optimize_constrained(&p, &[Variable::Thickness { surface: 2 }], &limits, &flag()).unwrap();
    assert!(!c.accepted);
    assert!(c.checks.iter().filter(|c| !c.passed).count() >= 4);
    assert!(
        !c.checks
            .iter()
            .find(|c| c.name == "efl_error_mm")
            .unwrap()
            .passed
    );
}
#[test]
fn minimum_thickness_cannot_be_certified_from_sparse_samples_alone() {
    let mut p = examples().remove(0);
    // A narrow dip sits between all 257 display samples, while the glass remains
    // physically separated. A requested clearance must still be rejected.
    p.surfaces.remove(0);
    p.stop = 0;
    p.surfaces[0].curvature = 0.;
    p.surfaces[0].thickness = 1. + 1e-8;
    p.surfaces[1].curvature = 0.;
    let a = 5.23_f64.powi(2);
    p.surfaces[1].asphere = [(a * a - 0.0001) * 1e-5, -2. * a * 1e-5, 1e-5, 0.];
    for s in &mut p.surfaces {
        s.semi_aperture = 10.;
    }
    for i in 0..=256 {
        assert!(p.surfaces[0].thickness + p.surfaces[1].sag(10. * i as f64 / 256.).unwrap().0 > 1.);
    }
    assert!(p.validate().is_empty());
    let limits = Constraints {
        min_edge_mm: 1.,
        ..constraints(&examples()[0])
    };
    assert!(constraint_checks(&p, &limits)
        .iter()
        .any(|c| c.name == "body_min_mm" && !c.passed));
}
#[test]
fn invalid_constraints_and_failed_rays_do_not_become_zero_merit() {
    let mut p = examples().remove(0);
    let mut c = constraints(&p);
    c.efl_tolerance_mm = 0.;
    assert!(optimize_constrained(&p, &[Variable::Thickness { surface: 2 }], &c, &flag()).is_err());
    c = constraints(&p);
    p.surfaces[1].semi_aperture = 0.001;
    assert!(optimize_constrained(&p, &[Variable::Thickness { surface: 2 }], &c, &flag()).is_err());
}
