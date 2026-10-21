use std::sync::atomic::AtomicBool;
use wollaston::*;
fn flag() -> AtomicBool {
    AtomicBool::new(false)
}
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() < t, "{a} != {b} (tol {t})");
}
#[test]
fn snell_and_reverse() {
    let d = V3::new(0., 0.5, (0.75f64).sqrt());
    let n = V3::new(0., 0., -1.);
    let t = refract(d, n, 1., 1.5).unwrap();
    close(t.y, 1. / 3., 1e-12);
    let reverse = refract(-t, -n, 1.5, 1.).unwrap();
    close((reverse + d).norm(), 0., 1e-12);
}
#[test]
fn total_internal_reflection() {
    assert!(refract(
        V3::new(0., 0.9, (0.19f64).sqrt()),
        V3::new(0., 0., -1.),
        1.5,
        1.
    )
    .is_none());
}
#[test]
fn spherical_intersection_analytic() {
    let p = examples().remove(0);
    let s = &p.surfaces[1];
    let (q, _, _) = intersect(s, 2., V3::new(0., 5., -1.), V3::new(0., 0., 1.)).unwrap();
    close(q.z, 2. + 50. - (2500. - 25f64).sqrt(), 1e-9);
}
#[test]
fn plate_preserves_direction_and_opl() {
    let mut p = examples().remove(0);
    p.surfaces.remove(0);
    p.stop = 0;
    for s in &mut p.surfaces {
        s.curvature = 0.;
    }
    p.materials[1].sellmeier = None;
    p.materials[1].n = 1.5;
    let d = V3::new(0., 0.2, (0.96f64).sqrt());
    let r = trace_ray(&p, V3::new(0., 0., -1.), d, 587.5618, false, None);
    assert_eq!(r.status, RayStatus::Complete);
    close((V3::from(r.direction) - d).norm(), 0., 1e-12);
    let expected = 1. / d.z + 1.5 * 4. / (1. - (0.2f64 / 1.5).powi(2)).sqrt() + 46. / d.z;
    close(r.optical_path, expected, 1e-9);
}
#[test]
fn thick_lens_focal_length() {
    let mut p = examples().remove(0);
    p.materials[1].sellmeier = None;
    p.materials[1].n = 1.5;
    let a = paraxial(&p, 587.5618).unwrap();
    let power = 0.5 * (1. / 50. - 1. / -50. + 0.5 * 4. / (1.5 * 50. * -50.));
    close(a.efl.unwrap(), 1. / power, 1e-10);
    close(a.entrance_pupil_radius.unwrap(), 6., 1e-12);
    close(a.f_number.unwrap(), a.efl.unwrap() / 12., 1e-12);
}
#[test]
fn paraxial_matches_small_ray_focus() {
    let p = examples().remove(0);
    let a = paraxial(&p, 587.5618).unwrap();
    let r = aimed_ray(&p, 0., 587.5618, [0., 1e-5]);
    assert_eq!(r.status, RayStatus::Complete);
    let q = r.points[r.points.len() - 2];
    let focus = q[2] - q[1] * r.direction[2] / r.direction[1];
    close(focus - p.vertices().last().unwrap(), a.bfl.unwrap(), 1e-6);
}
#[test]
fn symmetry_and_stop_aiming() {
    let p = examples().remove(2);
    for f in [0., 0.03, 0.1] {
        let r = aimed_ray(&p, f, 587.5618, [1., 2.]);
        assert_eq!(r.status, RayStatus::Complete);
        close(r.points[p.stop + 1][0], 1., 1e-7);
        close(r.points[p.stop + 1][1], 2., 1e-7);
    }
    let a = aimed_ray(&p, 0., 587.5618, [1., 2.]);
    let b = aimed_ray(&p, 0., 587.5618, [-1., -2.]);
    let aa = a.points.last().unwrap();
    let bb = b.points.last().unwrap();
    close(aa[0], -bb[0], 1e-9);
    close(aa[1], -bb[1], 1e-9);
}
#[test]
fn blocked_is_not_valid_spot() {
    let mut p = examples().remove(0);
    p.surfaces[1].semi_aperture = 1.;
    let a = analyze(&p, 9, &flag()).unwrap();
    assert!(a.spots[0].failed > 0);
    assert!(autofocus(&p, &flag()).is_err());
}
#[test]
fn invalid_material_and_crossing_surfaces() {
    let mut p = examples().remove(0);
    p.wavelengths[0].nm = 200.;
    assert!(!p.validate().is_empty());
    p = examples().remove(0);
    p.surfaces[1].thickness = 0.;
    assert!(p.validate().iter().any(|d| d.message.contains("intersect")));
}
#[test]
fn focus_improves_and_preserves_input() {
    let p = examples().remove(0);
    let saved = serde_json::to_string(&p).unwrap();
    let c = autofocus(&p, &flag()).unwrap();
    assert!(c.after_rms_um < c.before_rms_um);
    assert_eq!(saved, serde_json::to_string(&p).unwrap());
}
#[test]
fn cancellation() {
    assert!(analyze(&examples()[0], 17, &AtomicBool::new(true))
        .unwrap_err()
        .contains("Cancelled"));
}
#[test]
fn projects_roundtrip_and_examples_validate() {
    for p in examples() {
        assert!(p.validate().is_empty(), "{:?}", p.validate());
        let encoded = serde_json::to_string(&p).unwrap();
        let decoded: Project = serde_json::from_str(&encoded).unwrap();
        assert_eq!(encoded, serde_json::to_string(&decoded).unwrap());
    }
}
#[test]
fn asphere_slope_matches_finite_difference() {
    let mut s = examples()[0].surfaces[1].clone();
    s.conic = -0.7;
    s.asphere = [1e-5, -1e-8, 0., 0.];
    let (z, d) = s.sag(3.).unwrap();
    let fd = (s.sag(3. + 1e-5).unwrap().0 - s.sag(3. - 1e-5).unwrap().0) / 2e-5;
    assert!(z.is_finite());
    close(d, fd, 1e-8);
}
#[test]
fn ideal_circular_pupil_mtf() {
    use rustfft::num_complex::Complex;
    let n = 512;
    let radius = 64.;
    let pupil = (0..n * n)
        .map(|i| {
            let x = (i % n) as f64 - n as f64 / 2.;
            let y = (i / n) as f64 - n as f64 / 2.;
            Complex::new(
                if x * x + y * y <= radius * radius {
                    1.
                } else {
                    0.
                },
                0.,
            )
        })
        .collect();
    let (psf, s, t) = wave::pupil_to_psf(pupil, n);
    close(psf.iter().sum(), 1., 1e-10);
    let mut worst: f64 = 0.;
    for i in 0..128 {
        let v = i as f64 / 128.;
        let exact = 2. / std::f64::consts::PI * (v.acos() - v * (1. - v * v).sqrt());
        worst = worst.max((s[i] - exact).abs());
        close(s[i], t[i], 1e-10);
    }
    assert!(worst < 0.01, "MTF max error {worst}");
}
#[test]
fn optimized_candidate_has_valid_geometry() {
    let p = examples().remove(0);
    let c = optimize(
        &p,
        &[Variable::Thickness { surface: 2 }],
        paraxial(&p, 587.5618).unwrap().efl.unwrap(),
        &flag(),
    )
    .unwrap();
    assert!(c.project.validate().is_empty());
    assert!(c.after_rms_um <= c.before_rms_um);
}
#[test]
fn polychromatic_rms_includes_lateral_color() {
    let mut p = examples().remove(0);
    p.fields = vec![0.1];
    let a = analyze(&p, 9, &flag()).unwrap();
    let mono = (a
        .spots
        .iter()
        .map(|s| s.rms_um.unwrap().powi(2) * s.weight)
        .sum::<f64>()
        / a.spots.iter().map(|s| s.weight).sum::<f64>())
    .sqrt();
    assert!(a.fields[0].polychromatic_rms_um.unwrap() > mono);
    assert!(a.fields[0].lateral_color_um.unwrap() > 0.);
}
#[test]
fn no_surviving_rays_has_no_rms() {
    let mut p = examples().remove(0);
    p.surfaces[1].semi_aperture = 0.001;
    p.fields = vec![0.1];
    let a = analyze(&p, 7, &flag()).unwrap();
    assert!(a.fields[0].polychromatic_rms_um.is_none());
}
#[test]
fn finite_object_and_concave_front() {
    let mut p = examples().remove(0);
    p.surfaces.remove(0);
    p.stop = 0;
    p.surfaces[0].curvature = -0.02;
    p.surfaces[1].curvature = -0.04;
    p.surfaces[0].thickness = 5.;
    p.object_distance = Some(500.);
    assert!(p.validate().is_empty());
    let r = aimed_ray(&p, 0., 587.5618, [0., 5.]);
    assert_eq!(r.status, RayStatus::Complete);
}
#[test]
fn wave_energy_and_applicability() {
    let p = autofocus(&examples()[0], &flag()).unwrap().project;
    let w = wave::wave(&p, 0., 587.5618, &flag()).unwrap();
    close(w.psf.iter().sum(), 1., 1e-10);
    close(w.mtf_sagittal[0], 1., 1e-12);
    assert!(wave::wave(&p, 0.3, 587.5618, &flag()).is_err());
}
#[test]
fn planar_preview_is_derived_from_psf() {
    let mut p = examples().remove(0);
    p.fields = vec![0.];
    p = autofocus(&p, &flag()).unwrap().project;
    let image = preview::preview(&p, "grid", &flag()).unwrap();
    assert!(image.failures.is_empty());
    assert_eq!(image.frames[0].image.len(), 256 * 256);
    assert_ne!(image.frames[0].image, image.frames[0].source);
    close(image.frames[0].retained_energy, 1., 1e-10);
    assert!(preview::preview(&p, "grid", &AtomicBool::new(true)).is_err());
}
