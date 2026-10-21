use std::sync::atomic::AtomicBool;
use wollaston::*;
fn brief() -> synthesis::DesignBrief {
    serde_json::from_value(serde_json::json!({"structure":"tessar","focal_length_mm":50.,"f_number":8.,"sensor_mm":[6.,4.],"half_field_deg":5.,"max_length_mm":150.,"max_rms_um":100.,"iterations":2,"starts":1,"require_sensor_coverage":true,"exploration":"expanded","max_attempts":2})).unwrap()
}
#[test]
fn restoration_respects_locks_and_recovers_actual_power_pupil_and_clearance() {
    let mut p = synthesis::seed("tessar").unwrap();
    let focal = paraxial(&p, 587.5618).unwrap().efl.unwrap();
    let c = Constraints {
        target_efl_mm: focal * 1.1,
        efl_tolerance_mm: focal * 0.001,
        max_length_mm: 200.,
        min_center_mm: 5.,
        min_edge_mm: 1.,
        min_air_mm: 0.1,
        design: Some(DesignTargets {
            f_number: 8.,
            f_tolerance: 0.001,
            min_bfl_mm: 0.,
            max_front_diameter_mm: 100.,
        }),
    };
    let mut vars = vec![];
    for (i, s) in p.surfaces.iter().enumerate() {
        vars.push(VariableSetting {
            variable: Variable::Thickness { surface: i },
            min: Some(0.01),
            max: Some(150.),
            locked: i == p.surfaces.len() - 1,
        });
        if i != p.stop {
            vars.push(VariableSetting {
                variable: Variable::Curvature { surface: i },
                min: Some(-0.1),
                max: Some(0.1),
                locked: false,
            });
        }
        assert!(s.thickness > 0.);
    }
    vars.push(VariableSetting {
        variable: Variable::Aperture { surface: p.stop },
        min: Some(0.1),
        max: Some(10.),
        locked: false,
    });
    let image_gap = p.surfaces.last().unwrap().thickness;
    restoration::restore(&mut p, &vars, &c);
    let checks = constraint_checks(&p, &c);
    assert!(checks.iter().all(|v| v.passed), "{checks:?}");
    assert_eq!(image_gap, p.surfaces.last().unwrap().thickness);
    for v in &mut vars {
        v.locked = true;
    }
    let saved = serde_json::to_value(&p).unwrap();
    let mut impossible = c;
    impossible.target_efl_mm = 1000.;
    impossible.min_center_mm = 50.;
    restoration::restore(&mut p, &vars, &impossible);
    assert_eq!(serde_json::to_value(p).unwrap(), saved);
}
#[test]
fn search_checkpoints_resume_completed_units_and_reject_mismatched_tasks() {
    let b = brief();
    let flag = AtomicBool::new(false);
    let first = synthesis::step(&b, None, &flag, &mut |_| {}).unwrap();
    assert_eq!(first.checkpoint.cursor, 1);
    assert!(!first.checkpoint.candidates.is_empty());
    assert!(!first.done);
    let cp = serde_json::from_str(&serde_json::to_string(&first.checkpoint).unwrap()).unwrap();
    assert!(synthesis::step(&b, Some(&cp), &AtomicBool::new(true), &mut |_| {}).is_err());
    let resumed = synthesis::step(&b, Some(&cp), &flag, &mut |_| {}).unwrap();
    let direct = synthesis::run(&b, &flag, &mut |_| {}).unwrap();
    assert!(resumed.done);
    assert_eq!(resumed.checkpoint.cursor, 2);
    assert_eq!(resumed.outcome.candidates.len(), direct.candidates.len());
    for (a, b) in resumed.outcome.candidates.iter().zip(&direct.candidates) {
        assert_eq!(
            serde_json::to_value(&a.project).unwrap(),
            serde_json::to_value(&b.project).unwrap()
        );
        assert_eq!(a.after_rms_um, b.after_rms_um);
        assert_eq!(a.accepted, b.accepted);
    }
    let mut changed = b.clone();
    changed.f_number = 4.;
    assert!(synthesis::step(&changed, Some(&cp), &flag, &mut |_| {}).is_err());
    let mut corrupt = cp;
    corrupt.cursor = 900;
    assert!(synthesis::step(&b, Some(&corrupt), &flag, &mut |_| {}).is_err());
}
#[test]
fn planning_is_bounded_deterministic_and_keeps_legacy_defaults() {
    let mut b = brief();
    b.structure = "auto".into();
    b.element_glass = true;
    b.max_attempts = 40;
    let plan = synthesis::plan(&b).unwrap();
    assert_eq!(plan.len(), 40);
    assert_eq!(
        plan[..4]
            .iter()
            .map(|u| u.structure.as_str())
            .collect::<Vec<_>>(),
        vec!["tessar", "planar", "double_gauss", "sonnar"]
    );
    assert!(plan.iter().any(|u| u.glass > 0));
    assert!(plan.iter().any(|u| u.mutation == 3));
    assert_eq!(
        serde_json::to_value(&plan).unwrap(),
        serde_json::to_value(synthesis::plan(&b).unwrap()).unwrap()
    );
}
