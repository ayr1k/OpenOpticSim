use openoptics::{study::*, *};
use std::sync::atomic::AtomicBool;
fn example() -> (Project, StudyOptions) {
    let file: serde_json::Value =
        serde_json::from_str(include_str!("../../../examples/24-70mm-zoom.oos.json")).unwrap();
    let m = &file["workspace"]["motion"];
    let p = serde_json::from_value(m["reference"].clone()).unwrap();
    let options=serde_json::from_value(serde_json::json!({
        "mode":"scan","motion":{"groups":m["groups"],"positions_mm":[0,0],"object_distance":null,"min_air_mm":0.05,"target_f_number":null,"solve_group":null,"sampling":7,"criterion":"balanced","max_rms_um":30},
        "configurations":m["configurations"],"steps":2,"max_points":15,"adaptive":false,"refine_rms_um":1,
        "driver_group":0,"compensator_group":1,"near_mm":10000,"far_mm":100000,"include_infinity":false,
        "verification_sampling":17,"max_degradation_percent":0,"efl_tolerance_percent":null,"min_glass_mm":0,"require_sensor_coverage":true,"variables":[],"iterations":1
    })).unwrap();
    (p, options)
}
#[test]
fn scan_keeps_failed_points_and_matches_existing_motion() {
    let (p, mut o) = example();
    o.configurations[1].positions_mm[0] = 1000.;
    let r = run(&p, &o, &AtomicBool::new(false), &mut |_| {}).unwrap();
    assert_eq!(r.points.len(), 4);
    assert!(!r.accepted);
    assert!(r.points[1].rms_um.is_none());
    assert!(r.points[1].diagnostic.contains("行程"));
    assert_eq!(r.points[0].efl_mm.unwrap().round(), 24.);
    assert!(r.points[3].accepted);
    assert_eq!(r.reference.image_z(), p.image_z());
}
#[test]
fn bad_settings_and_cancellation_do_not_produce_candidates() {
    let (p, mut o) = example();
    assert!(run(&p, &o, &AtomicBool::new(true), &mut |_| {})
        .unwrap_err()
        .contains("Cancelled"));
    o.mode = "trajectory".into();
    o.compensator_group = 0;
    assert!(run(&p, &o, &AtomicBool::new(false), &mut |_| {}).is_err());
    o.mode = "scan".into();
    o.verification_sampling = 7;
    assert!(run(&p, &o, &AtomicBool::new(false), &mut |_| {}).is_err());
}
#[test]
fn optimization_preserves_sensor_and_protects_each_field() {
    let (p, mut o) = example();
    o.mode = "optimize".into();
    o.variables = vec![StudyVariable::Thickness {
        surface: 1,
        min: p.surfaces[1].thickness - 0.05,
        max: p.surfaces[1].thickness + 0.05,
    }];
    let r = run(&p, &o, &AtomicBool::new(false), &mut |_| {}).unwrap();
    assert!((r.reference.image_z() - p.image_z()).abs() < 1e-10);
    assert_eq!(r.before.len(), 4);
    assert_eq!(r.points.len(), 4);
    if r.accepted {
        for (a, b) in r.points.iter().zip(&r.before) {
            assert!(a.accepted);
            for (x, y) in a.fields_rms_um.iter().zip(&b.fields_rms_um) {
                assert!(x.unwrap() <= y.unwrap() + 1e-8);
            }
        }
    }
    o.variables.push(o.variables[0].clone());
    assert!(run(&p, &o, &AtomicBool::new(false), &mut |_| {}).is_err());
}
#[test]
fn target_tolerance_and_glass_constraints_cannot_be_hidden_by_average() {
    let (p, mut o) = example();
    o.efl_tolerance_percent = Some(0.01);
    o.configurations[0].target_efl_mm = Some(25.);
    let r = run(&p, &o, &AtomicBool::new(false), &mut |_| {}).unwrap();
    assert!(!r.points[0].accepted);
    assert!(r.points[0].diagnostic.contains("焦距偏差"));
    o.min_glass_mm = 100.;
    let r = run(&p, &o, &AtomicBool::new(false), &mut |_| {}).unwrap();
    assert!(r.points.iter().all(|p| !p.accepted));
}
#[test]
fn adaptive_scan_retains_failed_endpoints_and_respects_budget() {
    let (p, mut o) = example();
    o.configurations.truncate(2);
    o.configurations[1].positions_mm[0] = 1000.;
    o.adaptive = true;
    o.max_points = 5;
    let r = run(&p, &o, &AtomicBool::new(false), &mut |_| {}).unwrap();
    assert_eq!(r.points.len(), 5);
    assert_eq!(r.points.first().unwrap().coordinate, 0.);
    assert_eq!(r.points.last().unwrap().coordinate, 1.);
    assert!(!r.accepted);
}

#[test]
fn compensation_recovers_from_a_colliding_seed_without_bypassing_checks() {
    let (p, mut o) = example();
    o.mode = "trajectory".into();
    o.configurations.truncate(2);
    for c in &mut o.configurations {
        c.positions_mm[1] = -39.;
    }
    let r = run(&p, &o, &AtomicBool::new(false), &mut |_| {}).unwrap();
    assert!(r.accepted);
    assert!(r
        .points
        .iter()
        .all(|p| p.failed_rays == 0 && p.min_air_mm.unwrap() >= 0.05));
    assert!(r
        .points
        .iter()
        .all(|p| p.configuration.positions_mm[1] > -39.));
}

#[test]
fn cancellation_from_progress_never_returns_a_partial_candidate() {
    use std::sync::atomic::Ordering;
    let (p, o) = example();
    let cancel = AtomicBool::new(false);
    let r = run(&p, &o, &cancel, &mut |_| {
        cancel.store(true, Ordering::Relaxed);
    });
    assert_eq!(r.unwrap_err(), "Cancelled");
}
