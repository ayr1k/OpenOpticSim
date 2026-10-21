use std::sync::atomic::AtomicBool;
use wollaston::{aperture::*, *};
fn flag() -> AtomicBool {
    AtomicBool::new(false)
}
fn repaired() -> Project {
    serde_json::from_str(include_str!("../../../examples/400mm-repaired.oos.json")).unwrap()
}
#[test]
fn aperture_conversion_uses_entrance_pupil_and_does_not_mutate_the_source() {
    let p = repaired();
    let before = serde_json::to_value(&p).unwrap();
    for f in [4., 8., 16., 22.] {
        let q = at_f_number(&p, f).unwrap();
        assert!((paraxial(&q, 587.5618).unwrap().f_number.unwrap() - f).abs() < 1e-10);
        assert_eq!(q.surfaces[0].semi_aperture, p.surfaces[0].semi_aperture);
    }
    assert_eq!(serde_json::to_value(p).unwrap(), before);
}
#[test]
fn fixed_aperture_scan_matches_every_direct_analysis_and_preserves_failure_counts() {
    let p = repaired();
    let o = ApertureOptions {
        wave_method: wave::WaveMethod::Fast,
        f_numbers: vec![4., 16., 22.],
        refocus: false,
        sampling: 9,
        mtf: false,
    };
    let s = aperture_scan(&p, &o, &flag()).unwrap();
    assert!(s.points[0].failed > 0);
    assert!(s.points[0].score_um.is_none());
    for point in &s.points {
        let q = at_f_number(&p, point.requested_f_number).unwrap();
        let a = analyze(&q, o.sampling, &flag()).unwrap();
        assert_eq!(
            a.spots.iter().map(|s| s.failed).sum::<usize>(),
            point.failed
        );
        assert_eq!(
            a.fields
                .iter()
                .map(|f| f.polychromatic_rms_um)
                .collect::<Vec<_>>(),
            point.fields_rms_um
        );
        assert_eq!(
            point.image_distance_mm,
            p.surfaces.last().unwrap().thickness
        );
    }
    assert!(s.best_index.is_some());
}
#[test]
fn refocus_each_aperture_matches_standalone_focus_and_failed_focus_is_not_scored() {
    let p = repaired();
    let o = ApertureOptions {
        wave_method: wave::WaveMethod::Fast,
        f_numbers: vec![4., 16.],
        refocus: true,
        sampling: 9,
        mtf: false,
    };
    let s = aperture_scan(&p, &o, &flag()).unwrap();
    assert!(s.points[0].score_um.is_none());
    assert!(s.points[0].failed > 0);
    assert!(s.points[0].diagnostic.is_some());
    let q = autofocus(&at_f_number(&p, 16.).unwrap(), &flag())
        .unwrap()
        .project;
    assert_eq!(
        q.surfaces.last().unwrap().thickness,
        s.points[1].image_distance_mm
    );
    assert_eq!(
        analyze(&q, 9, &flag()).unwrap().fields[2].polychromatic_rms_um,
        s.points[1].fields_rms_um[2]
    );
}
#[test]
fn aperture_mtf_is_the_same_converged_wave_result_at_the_selected_image_plane() {
    let p = repaired();
    let s = aperture_scan(
        &p,
        &ApertureOptions {
            wave_method: wave::WaveMethod::Fast,
            f_numbers: vec![16.],
            refocus: false,
            sampling: 9,
            mtf: true,
        },
        &flag(),
    )
    .unwrap();
    let q = at_f_number(&p, 16.).unwrap();
    let w = wave::diffraction(
        &q,
        &wave::WaveOptions {
            method: wave::WaveMethod::Fast,
            field_index: 2,
            wavelength_nm: None,
            max_pupil_samples: 256,
        },
        &flag(),
    )
    .unwrap();
    let x = 30. / w.frequency_step_lp_mm;
    let i = x.floor() as usize;
    let expected = w.mtf_sagittal[i] + (w.mtf_sagittal[i + 1] - w.mtf_sagittal[i]) * (x - i as f64);
    assert_eq!(s.points[0].mtf[2].values.unwrap()[2], Some(expected));
    assert!(s.points[0]
        .mtf
        .iter()
        .all(|m| m.quality.as_ref().is_some_and(|q| q.converged)));
}
#[test]
fn pupil_map_matches_stop_shooting_and_keeps_unresolved_paths_empty() {
    let p: Project =
        serde_json::from_str(include_str!("fixtures/telephoto-original.oos.json")).unwrap();
    let m = stop_map(&p, 0, 587.5618, 9, &flag()).unwrap();
    assert!(m.samples.iter().any(|s| s.ray.stage == TraceStage::Aiming));
    for sample in m.samples {
        let direct = aimed_ray(&p, 0., 587.5618, sample.target_mm);
        assert_eq!(sample.ray.status, direct.status);
        if sample.ray.stage == TraceStage::Aiming {
            assert!(sample.ray.points.is_empty())
        } else {
            assert_eq!(sample.ray.points, direct.points);
        }
    }
}
#[test]
fn group_translation_keeps_internal_prescription_external_vertices_and_sensor_fixed() {
    let p = repaired();
    let g = GroupMove {
        start: 2,
        end: 5,
        delta_mm: 0.01,
    };
    let q = move_group(&p, &g).unwrap();
    let z = p.vertices();
    let zz = q.vertices();
    for i in 0..p.surfaces.len() {
        let expected = if (g.start..=g.end).contains(&i) {
            0.01
        } else {
            0.
        };
        assert!((zz[i] - z[i] - expected).abs() < 1e-12);
        assert_eq!(q.surfaces[i].curvature, p.surfaces[i].curvature);
        assert_eq!(q.surfaces[i].material, p.surfaces[i].material);
        if i != g.start - 1 && i != g.end {
            assert_eq!(q.surfaces[i].thickness, p.surfaces[i].thickness);
        }
    }
    assert!((p.image_z() - q.image_z()).abs() < 1e-12);
    let restored = move_group(
        &q,
        &GroupMove {
            delta_mm: -0.01,
            ..g
        },
    )
    .unwrap();
    assert!((restored.surfaces[1].thickness - p.surfaces[1].thickness).abs() < 1e-12);
    assert!(move_group(
        &p,
        &GroupMove {
            start: 0,
            end: 5,
            delta_mm: 1.
        }
    )
    .is_err());
    assert!(move_group(
        &p,
        &GroupMove {
            start: 2,
            end: 5,
            delta_mm: 1.
        }
    )
    .is_err());
    assert!(move_group(
        &p,
        &GroupMove {
            start: 3,
            end: 5,
            delta_mm: 0.01
        }
    )
    .is_err());
}
#[test]
fn design_constraints_and_aperture_variable_use_the_same_acceptance_contract() {
    let p = repaired();
    let mut c = Constraints {
        target_efl_mm: 400.,
        efl_tolerance_mm: 0.1,
        max_length_mm: 600.,
        min_center_mm: 0.25,
        min_edge_mm: 0.02,
        min_air_mm: 0.02,
        design: Some(DesignTargets {
            f_number: 16.,
            f_tolerance: 0.1,
            min_bfl_mm: 1.,
            max_front_diameter_mm: 70.,
        }),
    };
    assert!(constraint_checks(&p, &c).iter().all(|v| v.passed));
    c.design.as_mut().unwrap().f_number = 2.8;
    assert!(constraint_checks(&p, &c)
        .iter()
        .any(|v| v.name == "f_number_error" && !v.passed));
    c.design.as_mut().unwrap().f_number = 17.;
    let r = p.surfaces[p.stop].semi_aperture;
    let candidate = optimize_configured(
        &p,
        &OptimizationOptions {
            quality: None,
            staged: false,
            refocus_each_stage: false,
            variables: vec![VariableSetting {
                variable: Variable::Aperture { surface: p.stop },
                min: Some(r * 0.9),
                max: Some(r),
                locked: false,
            }],
            preset: AnalysisPreset::Balanced,
            max_iterations: 25,
        },
        &c,
        &flag(),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(
        candidate.accepted,
        candidate.checks.iter().all(|c| c.passed)
    );
    assert!(candidate.accepted);
    assert!(
        (paraxial(&candidate.project, 587.5618)
            .unwrap()
            .f_number
            .unwrap()
            - 17.)
            .abs()
            < 0.1
    );
}
#[test]
fn new_tasks_reject_invalid_settings_and_cancel_without_partial_results() {
    let p = repaired();
    let mut o = ApertureOptions {
        wave_method: wave::WaveMethod::Fast,
        f_numbers: vec![0.],
        refocus: false,
        sampling: 9,
        mtf: false,
    };
    assert!(aperture_scan(&p, &o, &flag()).is_err());
    o.f_numbers = vec![16.];
    assert!(aperture_scan(&p, &o, &AtomicBool::new(true)).is_err());
    assert!(stop_map(&p, 0, 587.5618, 9, &AtomicBool::new(true)).is_err());
    assert!(stop_map(&p, 99, 587.5618, 9, &flag()).is_err());
    let mut empty = p.clone();
    empty.surfaces.clear();
    empty.stop = 0;
    assert!(aperture_scan(&empty, &o, &flag()).is_err());
    assert!(stop_map(&empty, 0, 587.5618, 9, &flag()).is_err());
}
