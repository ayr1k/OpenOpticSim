use openoptics::*;
use std::sync::atomic::AtomicBool;
#[test]
fn sensor_guard_rejects_narrow_sampling_and_accepts_actual_checkpoints() {
    let mut p: Project =
        serde_json::from_str(include_str!("../../../examples/35mm-coverage.oos.json")).unwrap();
    let flag = AtomicBool::new(false);
    let guards = quality::QualityGuards {
        require_sensor_coverage: true,
        ..Default::default()
    };
    let a = analyze(&p, 17, &flag).unwrap();
    assert!(quality::checks(&a, &a, &guards).iter().all(|c| c.passed));
    p.field_spec = None;
    p.fields = vec![0., 0.05, 0.1];
    let narrow = analyze(&p, 17, &flag).unwrap();
    assert_eq!(
        quality::checks(&narrow, &narrow, &guards)
            .iter()
            .filter(|c| !c.passed)
            .count(),
        3
    );
}
#[test]
fn legacy_brief_defaults_do_not_silently_enable_glass_or_coverage_search() {
    let b: synthesis::DesignBrief = serde_json::from_value(serde_json::json!({
        "structure":"tessar","focal_length_mm":50.,"f_number":8.,"sensor_mm":[36.,24.],
        "half_field_deg":5.,"max_length_mm":100.,"max_rms_um":25.,"iterations":1,"starts":1
    }))
    .unwrap();
    assert!(!b.require_sensor_coverage);
    assert!(!b.glass_search);
}
