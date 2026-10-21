use std::sync::atomic::{AtomicBool, Ordering};
use wollaston::*;
fn flag() -> AtomicBool {
    AtomicBool::new(false)
}
fn wide() -> Project {
    serde_json::from_str(include_str!("../../../examples/35mm-coverage.oos.json")).unwrap()
}
#[test]
fn sensor_height_targets_solve_actual_distorted_image_and_follow_sensor_and_focus() {
    let p = wide();
    assert!(p.validate_optical().is_empty());
    let plan = fields::plan(&p, &flag()).unwrap();
    assert_eq!(plan.fields.len(), 4);
    for r in &plan.fields {
        assert!(r.residual_mm.unwrap().abs() < 1e-6);
        assert_eq!(r.chief_status, "complete");
    }
    assert!(
        (plan.fields[3].angle_rad.unwrap() - plan.fields[3].initial_angle_rad.unwrap()).abs()
            > 0.01
    );
    let a = analyze(&p, 17, &flag()).unwrap();
    let c = a.coverage.unwrap();
    assert!(c.checkpoints.iter().all(|c| c.status == "sampled_clear"));
    assert!((c.max_tested_height_mm.unwrap() - 36f64.hypot(24.) / 2.).abs() < 1e-6);
    let mut q = p.clone();
    q.sensor = [24., 16.];
    let b = fields::plan(&q, &flag()).unwrap();
    assert!(b.fields[3].angle_rad < plan.fields[3].angle_rad);
    q = p.clone();
    q.surfaces.last_mut().unwrap().thickness += 2.;
    let b = fields::plan(&q, &flag()).unwrap();
    assert!(b.fields[3].angle_rad < plan.fields[3].angle_rad);
    assert_eq!(p.fields, wide().fields);
}
#[test]
fn legacy_narrow_fields_and_larger_but_unmatched_fields_never_certify_corner() {
    let mut p = wide();
    p.field_spec = None;
    p.fields = vec![0., 0.07, 0.1];
    let c = analyze(&p, 9, &flag()).unwrap().coverage.unwrap();
    assert!(c.max_tested_height_mm.unwrap() < 4.);
    assert!(c.checkpoints.iter().all(|c| c.status == "untested"));
    p.fields.push(40f64.to_radians());
    let c = analyze(&p, 9, &flag()).unwrap().coverage.unwrap();
    assert!(c.max_tested_height_mm.unwrap() > c.half_diagonal_mm);
    assert_eq!(c.checkpoints[2].status, "untested");
    assert_eq!(c.fields.last().unwrap().fast_mtf, "outside_model");
}
#[test]
fn unreachable_height_physical_clipping_and_aiming_failure_are_distinct() {
    let mut p = wide();
    p.field_spec = Some(fields::FieldSpec::ImageHeights {
        heights_mm: vec![0., 100.],
    });
    p.fields = vec![0., 0.5];
    let plan = fields::plan(&p, &flag()).unwrap();
    assert!(plan.fields[1].angle_rad.is_none());
    assert!(analyze(&p, 9, &flag()).is_err());
    p = wide();
    p.surfaces[1].semi_aperture = 0.1;
    let c = analyze(&p, 9, &flag()).unwrap().coverage.unwrap();
    assert!(c.fields.iter().any(|f| f.blocked > 0));
    assert!(c.fields.iter().all(|f| f.unresolved == 0));
    let flag = flag();
    flag.store(true, Ordering::Relaxed);
    assert_eq!(fields::plan(&wide(), &flag).unwrap_err(), "Cancelled");
}
#[test]
fn new_field_schema_roundtrips_and_angles_outside_verified_boundary_are_rejected() {
    let p = wide();
    let copy: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&p).unwrap(),
        serde_json::to_value(&copy).unwrap()
    );
    let mut q = p.clone();
    q.schema_version = 2;
    assert!(!q.validate().is_empty());
    q = p.clone();
    q.fields[0] = 0.701;
    assert!(!q.validate().is_empty());
    q = p;
    q.field_spec = Some(fields::FieldSpec::ImageHeights {
        heights_mm: vec![0.],
    });
    assert!(!q.validate().is_empty());
}
#[test]
fn direct_off_axis_diffraction_has_flat_sensor_scale_and_sampling_evidence() {
    let mut p = wide();
    p.field_spec = None;
    p.fields = vec![32f64.to_radians(), 40f64.to_radians()];
    assert!(
        wave::diffraction(&p, &wave::WaveOptions::default(), &flag())
            .unwrap_err()
            .contains("applicability")
    );
    for i in 0..2 {
        let w = wave::diffraction(
            &p,
            &wave::WaveOptions {
                method: wave::WaveMethod::SphereKirchhoff,
                field_index: i,
                ..Default::default()
            },
            &flag(),
        )
        .unwrap();
        assert_eq!(w.size, 256);
        assert!(w.quality.converged);
        assert!(w.quality.previous_sampling < w.quality.pupil_sampling);
        assert!(w.quality.mtf_max_change <= 0.015);
        assert!(w.quality.psf_l1_change <= 0.04);
        assert!(w.quality.edge_energy <= 0.005);
        assert!((w.psf.iter().sum::<f64>() - 1.).abs() < 1e-11);
        assert!((w.frequency_step_lp_mm * 256. * w.pixel_um / 1000. - 1.).abs() < 1e-12);
        let c = w.coverage.unwrap();
        assert_eq!(c.fields[i].wide_mtf, "converged");
        assert_eq!(c.fields[i].fast_mtf, "outside_model");
    }
}
#[test]
fn individual_field_guards_block_a_better_average_and_record_missing_metrics() {
    let p = wide();
    let before = analyze(&p, 9, &flag()).unwrap();
    let mut after = before.clone();
    after.fields[0].polychromatic_rms_um =
        Some(before.fields[0].polychromatic_rms_um.unwrap() * 2.);
    after.fields[3].polychromatic_rms_um = Some(0.1);
    let g = quality::QualityGuards {
        max_degradation_percent: Some(0.),
        max_worst_rms_um: Some(1.),
        max_distortion_percent: Some(1.),
        max_lateral_color_um: Some(1.),
        min_geometric_fraction: Some(1.),
        ..Default::default()
    };
    let checks = quality::checks(&after, &before, &g);
    assert!(checks
        .iter()
        .any(|c| !c.passed && c.name.contains("视场 1")));
    assert!(checks.iter().any(|c| !c.passed && c.name.contains("最差")));
    let mut finite = after.clone();
    finite
        .spots
        .iter_mut()
        .for_each(|s| s.distortion_percent = None);
    assert!(quality::checks(&finite, &before, &g)
        .iter()
        .any(|c| !c.passed && c.name.contains("畸变")));
}
#[test]
fn staged_search_compares_every_stage_with_original_and_uses_dense_final_guards() {
    let mut p = examples().remove(0);
    p.surfaces[0].semi_aperture = 1.;
    let c = Constraints {
        target_efl_mm: paraxial(&p, 587.5618).unwrap().efl.unwrap(),
        efl_tolerance_mm: 1.,
        max_length_mm: 200.,
        min_center_mm: 0.,
        min_edge_mm: 0.,
        min_air_mm: 0.,
        design: None,
    };
    let o = OptimizationOptions {
        variables: vec![VariableSetting {
            variable: Variable::Thickness { surface: 2 },
            min: Some(20.),
            max: Some(100.),
            locked: false,
        }],
        preset: AnalysisPreset::Balanced,
        max_iterations: 2,
        quality: Some(quality::QualityGuards {
            max_degradation_percent: Some(100.),
            ..Default::default()
        }),
        staged: true,
        refocus_each_stage: false,
    };
    let result = optimize_configured(&p, &o, &c, &flag(), &mut |_| {}).unwrap();
    assert!(result.accepted);
    let r = result.report.unwrap();
    assert_eq!(r.stages.len(), 3);
    assert_eq!(r.stages.last().unwrap().field_scale, 1.);
    for stage in &r.stages {
        let mut original = p.clone();
        original
            .fields
            .iter_mut()
            .for_each(|f| *f *= stage.field_scale);
        let a = analyze(&original, 33, &flag()).unwrap();
        assert_eq!(
            stage.original_fields_rms_um,
            a.fields
                .iter()
                .map(|f| f.polychromatic_rms_um)
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(result.project.fields, p.fields);
    let mut refocused = o.clone();
    refocused.refocus_each_stage = true;
    let result = optimize_configured(&p, &refocused, &c, &flag(), &mut |_| {}).unwrap();
    assert!(result.accepted);
    assert!(result
        .report
        .as_ref()
        .unwrap()
        .focus_condition
        .contains("independently refocused"));
    assert!(result
        .checks
        .iter()
        .filter(|c| c.name == "bound_thickness")
        .all(|c| c.actual == Some(result.project.surfaces[2].thickness)));
}
#[test]
fn configured_optimizer_cannot_adopt_when_a_field_floor_fails() {
    let mut p = examples().remove(0);
    p.surfaces[0].semi_aperture = 1.;
    let c = Constraints {
        target_efl_mm: paraxial(&p, 587.5618).unwrap().efl.unwrap(),
        efl_tolerance_mm: 1.,
        max_length_mm: 200.,
        min_center_mm: 0.,
        min_edge_mm: 0.,
        min_air_mm: 0.,
        design: None,
    };
    let o = OptimizationOptions {
        variables: vec![VariableSetting {
            variable: Variable::Thickness { surface: 2 },
            min: Some(20.),
            max: Some(100.),
            locked: false,
        }],
        preset: AnalysisPreset::Balanced,
        max_iterations: 2,
        quality: Some(quality::QualityGuards {
            max_rms_um: vec![Some(0.); 3],
            ..Default::default()
        }),
        staged: false,
        refocus_each_stage: false,
    };
    let result = optimize_configured(&p, &o, &c, &flag(), &mut |_| {}).unwrap();
    assert!(!result.accepted);
    assert!(result
        .checks
        .iter()
        .any(|c| !c.passed && c.name.contains("RMS")));
    assert_eq!(result.accepted, result.checks.iter().all(|c| c.passed));
}
#[test]
fn sampled_throughput_constraint_allows_only_explicit_physical_clipping() {
    let mut p = wide();
    p.field_spec = None;
    p.fields = vec![0., 0.1];
    p.surfaces[1].semi_aperture = 0.3;
    let c = Constraints {
        target_efl_mm: 35.,
        efl_tolerance_mm: 1.,
        max_length_mm: 200.,
        min_center_mm: 0.,
        min_edge_mm: 0.,
        min_air_mm: 0.,
        design: None,
    };
    let o = OptimizationOptions {
        variables: vec![VariableSetting {
            variable: Variable::Thickness { surface: 2 },
            min: Some(20.),
            max: Some(100.),
            locked: false,
        }],
        preset: AnalysisPreset::Balanced,
        max_iterations: 1,
        quality: Some(quality::QualityGuards {
            min_geometric_fraction: Some(0.1),
            ..Default::default()
        }),
        staged: false,
        refocus_each_stage: false,
    };
    let result = optimize_configured(&p, &o, &c, &flag(), &mut |_| {}).unwrap();
    assert!(result.accepted);
    assert!(result
        .checks
        .iter()
        .any(|c| c.name == "numerical_failed_rays" && c.actual == Some(0.)));
    let a = analyze(&result.project, 33, &flag()).unwrap();
    assert!(a.spots.iter().any(|s| s.failed > 0));
    assert!(a
        .spots
        .iter()
        .flat_map(|s| &s.failures)
        .all(|f| f.status == RayStatus::Blocked));
}
