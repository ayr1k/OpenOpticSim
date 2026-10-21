use std::sync::atomic::AtomicBool;
use wollaston::*;
fn flag() -> AtomicBool {
    AtomicBool::new(false)
}
fn edges(p: &mut Project, radius: f64) {
    p.schema_version = 2;
    for i in 0..p.surfaces.len() {
        if p.surfaces[i].material == 0 && (i == 0 || p.surfaces[i - 1].material == 0) {
            continue;
        }
        p.surfaces[i].mechanical = Some(MechanicalEdge {
            semi_diameter: radius,
            shape: EdgeShape::Flat,
        });
    }
}
#[test]
fn empty_document_is_valid_but_cannot_run_optics() {
    let mut p = examples().remove(0);
    p.schema_version = 2;
    p.surfaces.clear();
    p.stop = 0;
    assert!(p.validate().is_empty());
    assert!(analyze(&p, 17, &flag()).is_err());
    assert!(paraxial(&p, 587.5618).is_err());
    assert!(autofocus(&p, &flag()).is_err());
    assert!(wave::diffraction(&p, &wave::WaveOptions::default(), &flag()).is_err());
    let request: Request =
        serde_json::from_value(serde_json::json!({"operation":"analyze","project":p})).unwrap();
    assert!(execute(request, &flag())
        .unwrap_err()
        .contains("Add a lens"));
}
#[test]
fn v1_and_v2_roundtrip_preserve_every_parameter() {
    for mut p in examples() {
        for mechanics in [false, true] {
            if mechanics {
                edges(&mut p, 14.);
            }
            let text = serde_json::to_string(&p).unwrap();
            let q: Project = serde_json::from_str(&text).unwrap();
            assert_eq!(serde_json::to_string(&q).unwrap(), text);
            assert!(q.validate().is_empty(), "{:?}", q.validate());
        }
    }
}
#[test]
fn mechanical_annulus_has_exact_corner_and_does_not_extend_optical_aperture() {
    let mut p = examples().remove(0);
    edges(&mut p, 16.);
    let s = &p.surfaces[2];
    assert_eq!(s.profile(12.), s.profile(16.));
    let r = trace_ray(
        &p,
        V3::new(0., 13., -1.),
        V3::new(0., 0., 1.),
        587.5618,
        true,
        None,
    );
    assert_eq!(r.status, RayStatus::Blocked);
    let a = serde_json::to_value(analyze(&examples()[0], 17, &flag()).unwrap()).unwrap();
    let b = serde_json::to_value(analyze(&p, 17, &flag()).unwrap()).unwrap();
    assert_eq!(a, b);
}
#[test]
fn mechanical_only_edits_leave_psf_mtf_and_imaging_identical() {
    let mut p = examples().remove(1);
    p.fields = vec![0.];
    p = autofocus(&p, &flag()).unwrap().project;
    let options = wave::WaveOptions::default();
    let before = wave::diffraction(&p, &options, &flag()).unwrap();
    let before_image = preview::preview_with_options(&p, "grid", &options, &flag()).unwrap();
    edges(&mut p, 14.);
    assert!(p.validate().is_empty());
    let after = wave::diffraction(&p, &options, &flag()).unwrap();
    let after_image = preview::preview_with_options(&p, "grid", &options, &flag()).unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
    assert_eq!(
        serde_json::to_value(before_image).unwrap(),
        serde_json::to_value(after_image).unwrap()
    );
}
#[test]
fn invalid_dimensions_extensions_and_mechanical_crossings_are_reported() {
    let mut p = examples().remove(0);
    edges(&mut p, 8.);
    assert!(p
        .validate()
        .iter()
        .any(|d| d.message.contains("Mechanical radius")));
    edges(&mut p, 30.);
    for s in p.surfaces.iter_mut().skip(1) {
        s.mechanical.as_mut().unwrap().shape = EdgeShape::Curved;
    }
    assert!(p
        .validate()
        .iter()
        .any(|d| d.message.contains("Geometry intersection")));
    for s in p.surfaces.iter_mut().skip(1) {
        s.mechanical.as_mut().unwrap().semi_diameter = 60.;
    }
    assert!(p
        .validate()
        .iter()
        .any(|d| d.message.contains("surface domain")));
}
#[test]
fn collision_across_small_stop_is_not_hidden() {
    let mut p = examples().remove(0);
    let mut face = p.surfaces[1].clone();
    face.curvature = 0.;
    face.semi_aperture = 10.;
    face.thickness = 4.;
    let mut rear = face.clone();
    rear.curvature = 0.02;
    rear.material = 0;
    rear.thickness = 0.5;
    let mut stop = rear.clone();
    stop.curvature = 0.;
    stop.semi_aperture = 0.5;
    let mut next = face.clone();
    next.curvature = -0.02;
    let mut last = face.clone();
    last.material = 0;
    last.thickness = 50.;
    p.surfaces = vec![face, rear, stop, next, last];
    p.stop = 2;
    let g = geometry::inspect(&p);
    assert!(g
        .spans
        .iter()
        .filter(|s| s.back == s.front + 1)
        .all(|s| s.certified));
    assert!(g
        .spans
        .iter()
        .any(|s| s.front == 1 && s.back == 3 && !s.certified));
    assert!(!p.validate().is_empty());
}
#[test]
fn narrow_aspheric_intersection_cannot_pass_a_sampling_only_check() {
    let mut p = examples().remove(0);
    p.surfaces.remove(0);
    p.stop = 0;
    p.surfaces[0].curvature = 0.;
    p.surfaces[0].thickness = 1e-8;
    p.surfaces[1].curvature = 0.;
    let a = 5.23f64.powi(2);
    p.surfaces[1].asphere = [(a * a - 0.0001) * 1e-5, -2. * a * 1e-5, 1e-5, 0.];
    for s in &mut p.surfaces {
        s.semi_aperture = 10.;
    }
    for i in 0..=256 {
        assert!(1e-8 + p.surfaces[1].sag(10. * i as f64 / 256.).unwrap().0 > 0.);
    }
    assert!(1e-8 + p.surfaces[1].sag(5.23).unwrap().0 < -1e-9);
    assert!(!geometry::inspect(&p).spans[0].certified);
    assert!(!p.validate().is_empty());
}
#[test]
fn changing_clear_aperture_changes_ray_clipping() {
    let mut p = examples().remove(0);
    edges(&mut p, 16.);
    let before = analyze(&p, 17, &flag()).unwrap();
    p.surfaces[1].semi_aperture = 2.;
    let after = analyze(&p, 17, &flag()).unwrap();
    assert_ne!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
}
