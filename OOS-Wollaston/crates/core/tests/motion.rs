use std::sync::atomic::AtomicBool;
use wollaston::{motion::*, *};
fn example() -> (Project, MotionOptions) {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../../../examples/24-70mm-zoom.oos.json")).unwrap();
    let m = &v["workspace"]["motion"];
    let p: Project = serde_json::from_value(m["reference"].clone()).unwrap();
    let o: MotionOptions = serde_json::from_value(serde_json::json!({
        "groups":m["groups"], "positions_mm":[0.,0.], "object_distance":null,
        "min_air_mm":0.05,"target_f_number":null,"solve_group":null,
        "sampling":7,"criterion":"balanced","max_rms_um":30.
    }))
    .unwrap();
    (p, o)
}
#[test]
fn first_group_moves_in_fixed_sensor_and_object_frame() {
    let (p, mut o) = example();
    o.positions_mm = vec![10., -2.];
    o.object_distance = Some(1000.);
    let (q, origin) = placed(&p, &o).unwrap();
    assert_eq!(origin, 10.);
    assert_eq!(q.object_distance, Some(1010.));
    assert!((q.image_z() + origin - p.image_z()).abs() < 1e-12);
    for i in [0, 1, 3, 4] {
        assert_eq!(q.surfaces[i].thickness, p.surfaces[i].thickness);
    }
    assert!((q.vertices()[3] + origin - (p.vertices()[3] - 2.)).abs() < 1e-12);
}
#[test]
fn invalid_travel_overlap_lock_and_clearance_are_rejected() {
    let (p, mut o) = example();
    o.positions_mm[0] = 91.;
    assert!(placed(&p, &o).is_err());
    o.positions_mm[0] = 1.;
    o.groups[0].locked = true;
    assert!(placed(&p, &o).is_err());
    o.groups[0].locked = false;
    o.groups[1].start = 0;
    assert!(placed(&p, &o).is_err());
    let (_, mut o) = example();
    o.min_air_mm = 1000.;
    assert!(placed(&p, &o).is_err());
}
#[test]
fn zoom_keyframes_match_requested_efl_and_fixed_plane() {
    let (p, mut o) = example();
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../../../examples/24-70mm-zoom.oos.json")).unwrap();
    for c in v["workspace"]["motion"]["configurations"]
        .as_array()
        .unwrap()
    {
        o.positions_mm = serde_json::from_value(c["positions_mm"].clone()).unwrap();
        let r = run(&p, &o, &AtomicBool::new(false)).unwrap();
        assert!(
            (r.analysis.paraxial.efl.unwrap() - c["target_efl_mm"].as_f64().unwrap()).abs() < 1e-8
        );
        assert!(r.accepted);
        assert!(r.analysis.spots.iter().all(|s| s.failed == 0));
        assert!((r.project.image_z() + r.origin_shift_mm - p.image_z()).abs() < 1e-10);
    }
}
#[test]
fn target_aperture_respects_reference_opening() {
    let (p, mut o) = example();
    o.target_f_number = Some(64.);
    let (q, _) = placed(&p, &o).unwrap();
    assert!((paraxial(&q, 587.5618).unwrap().f_number.unwrap() - 64.).abs() < 1e-10);
    o.target_f_number = Some(1.);
    assert!(placed(&p, &o).is_err());
}
#[test]
fn whole_lens_focus_agrees_with_thin_lens_limit_and_cancels() {
    let (mut p, _) = example();
    p.surfaces.truncate(2);
    for s in &mut p.surfaces {
        s.mechanical = None;
    }
    p.stop = 0;
    p.field_spec = None;
    p.schema_version = 1;
    p.fields = vec![0.];
    p.wavelengths = vec![Wavelength {
        nm: 587.5618,
        weight: 1.,
    }];
    p.materials[1].sellmeier = None;
    p.materials[1].n = 1.5;
    p.surfaces[0].curvature = 0.02;
    p.surfaces[0].thickness = 0.01;
    p.surfaces[0].semi_aperture = 0.1;
    p.surfaces[1].curvature = -0.02;
    p.surfaces[1].material = 0;
    p.surfaces[1].semi_aperture = 0.1;
    p.surfaces[1].thickness = paraxial(&p, 587.5618).unwrap().bfl.unwrap();
    let o = MotionOptions {
        groups: vec![MovingGroup {
            start: 0,
            end: 1,
            min_mm: -5.,
            max_mm: 0.,
            locked: false,
        }],
        positions_mm: vec![0.],
        object_distance: Some(1000.),
        min_air_mm: 0.,
        target_f_number: None,
        solve_group: Some(0),
        sampling: 7,
        criterion: FocusCriterion::Center,
        max_rms_um: 1.,
    };
    let r = run(&p, &o, &AtomicBool::new(false)).unwrap();
    // Fixed object at -1000, sensor initially at 50. Translation x obeys
    // 1/(1000+x) + 1/(50-x) = 1/50, hence x^2+950x+2500=0.
    let expected = (-950. + (950_f64.powi(2) - 10000.).sqrt()) / 2.;
    assert!((r.positions_mm[0] - expected).abs() < 0.005);
    assert!(r.accepted);
    assert!(run(&p, &o, &AtomicBool::new(true)).is_err());
}
