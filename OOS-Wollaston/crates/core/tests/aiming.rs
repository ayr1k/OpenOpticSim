use std::sync::atomic::AtomicBool;
use wollaston::*;
fn original() -> Project {
    serde_json::from_str(include_str!("fixtures/telephoto-original.oos.json")).unwrap()
}
#[test]
fn repaired_telephoto_forms_a_positive_400mm_image_without_failed_samples() {
    let p: Project =
        serde_json::from_str(include_str!("../../../examples/400mm-repaired.oos.json")).unwrap();
    let a = analyze(&p, 33, &AtomicBool::new(false)).unwrap();
    assert!((a.paraxial.efl.unwrap() - 400.).abs() < 0.01);
    assert!((a.paraxial.f_number.unwrap() - 16.).abs() < 0.001);
    assert!(a.paraxial.bfl.unwrap() > 380.);
    assert!(a.spots.iter().all(|s| s.failed == 0));
    assert!(a
        .fields
        .iter()
        .all(|f| f.polychromatic_rms_um.unwrap() < 16.));
}
#[test]
fn stable_chief_branch_matches_independent_bracketed_reference() {
    let p = original();
    let ray = aimed_ray(&p, 0., 587.5618, [0., 0.1]);
    assert_eq!(ray.status, RayStatus::Complete);
    assert!((ray.points[0][1] - 1.3902722311447748).abs() < 1e-6);
    assert!((ray.points[p.stop + 1][1] - 0.1).abs() < 1e-8);
    let ray = aimed_ray(&p, 0.5_f64.to_radians(), 587.5618, [0., 1.2]);
    assert_eq!(ray.status, RayStatus::Complete);
    assert!((ray.points[0][1] - 12.886096552975577).abs() < 1e-6);
    assert!(ray.aiming.unwrap().residual_mm.unwrap() < 1e-8);
}
#[test]
fn unresolved_or_folded_stop_target_never_returns_trial_geometry() {
    let p = original();
    for (field, target) in [
        (0., [0., 1.2]),
        (4.011_f64.to_radians(), [0., 0.]),
        (5.73_f64.to_radians(), [0.1, 0.1]),
    ] {
        let r = aimed_ray(&p, field, 587.5618, target);
        assert_eq!(r.status, RayStatus::AimingFailed);
        assert_eq!(r.stage, TraceStage::Aiming);
        assert_eq!(r.surface, Some(p.stop));
        assert!(r.points.is_empty());
        assert!(r.aiming.unwrap().failure.is_some());
    }
}
#[test]
fn successful_aiming_is_retraced_with_real_clipping_and_actual_termination() {
    let mut p = original();
    p.surfaces[0].semi_aperture = 0.5;
    let r = aimed_ray(&p, 0., 587.5618, [0., 0.1]);
    assert_eq!(r.status, RayStatus::Blocked);
    assert_eq!(r.stage, TraceStage::Trace);
    assert_eq!(r.surface, Some(0));
    assert_eq!(r.points.len(), 2);
    assert!(r.aiming.unwrap().failure.is_none());
}
#[test]
fn analysis_accounts_for_every_failure_and_keeps_aiming_and_physics_separate() {
    let mut p = original();
    p.fields = vec![0., 4.011_f64.to_radians()];
    let a = analyze(&p, 9, &AtomicBool::new(false)).unwrap();
    for spot in &a.spots {
        assert_eq!(
            spot.failed,
            spot.failures.iter().map(|f| f.count).sum::<usize>()
        );
    }
    for ray in &a.rays {
        if ray.stage == TraceStage::Aiming {
            assert!(ray.points.is_empty());
        }
        if ray.points.len() > 1 && ray.points[1][0].hypot(ray.points[1][1]) > 35. + 1e-8 {
            assert_eq!(ray.surface, Some(0));
            assert_eq!(ray.status, RayStatus::Blocked);
            assert_eq!(ray.points.len(), 2);
        }
    }
    let error: serde_json::Value = serde_json::from_str(&ray_failure_error(&a)).unwrap();
    assert_eq!(error["code"], "ray_failures");
    assert!(a
        .diagnostics
        .iter()
        .any(|d| d.message.starts_with("VirtualImage|")));
}
#[test]
fn autofocus_explains_virtual_image_and_expands_for_a_positive_distant_focus() {
    let p = original();
    let error = autofocus(&p, &AtomicBool::new(false)).unwrap_err();
    let error: serde_json::Value = serde_json::from_str(&error).unwrap();
    assert_eq!(error["code"], "virtual_image");
    assert!(error["bfl_mm"].as_f64().unwrap() < 0.);
    let mut p = examples().remove(0);
    p.surfaces.last_mut().unwrap().thickness = 2.;
    let candidate = autofocus(&p, &AtomicBool::new(false)).unwrap();
    assert!(candidate.project.surfaces.last().unwrap().thickness > 30.);
    assert!(candidate.after_rms_um < candidate.before_rms_um);
}
