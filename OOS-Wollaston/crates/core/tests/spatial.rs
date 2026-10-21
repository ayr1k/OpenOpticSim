use std::sync::atomic::AtomicBool;
use wollaston::{spatial::*, *};
fn project() -> Project {
    serde_json::from_str(include_str!("../../../examples/singlet.oos.json")).unwrap()
}
fn options() -> SpatialOptions {
    SpatialOptions {
        rays: vec![InputRay {
            origin_mm: [0.3, 0.7, -10.],
            direction: [0.001, 0.002, 1.],
            wavelength_nm: 587.5618,
        }],
        ..Default::default()
    }
}
#[test]
fn identity_matches_original_trace_and_thread_order() {
    let p = project();
    let mut o = options();
    o.rays = vec![o.rays[0].clone(); 200];
    let r = trace_spatial(&p, &o, &AtomicBool::new(false)).unwrap();
    let old = trace_ray(
        &p,
        V3::from(o.rays[0].origin_mm),
        V3::from(o.rays[0].direction),
        587.5618,
        true,
        None,
    );
    assert_eq!(r[0].status, old.status);
    assert_eq!(r[0].points.len(), old.points.len());
    for (a, b) in r[0].points.iter().zip(&old.points) {
        assert!((V3::from(*a) - V3::from(*b)).norm() < 1e-10);
    }
    assert!((r[0].optical_path_mm - old.optical_path).abs() < 1e-10);
    o.threads = 4;
    let parallel = trace_spatial(&p, &o, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        serde_json::to_value(r).unwrap(),
        serde_json::to_value(parallel).unwrap()
    );
}
#[test]
fn rigid_rotation_translation_are_covariant() {
    let p = project();
    let base = options();
    let a = trace_spatial(&p, &base, &AtomicBool::new(false)).unwrap();
    let angles = [0.02, -0.03, 0.04];
    let rot = nalgebra::Rotation3::from_euler_angles(angles[0], angles[1], angles[2]);
    let shift = V3::new(0.1, -0.2, 0.3);
    let mut o = base.clone();
    o.poses = p
        .vertices()
        .iter()
        .map(|z| {
            let v = V3::new(0., 0., *z);
            Pose {
                offset_mm: (rot * v + shift - v).into(),
                rotation_rad: angles,
            }
        })
        .collect();
    let v = V3::new(0., 0., p.image_z());
    o.detector = Pose {
        offset_mm: (rot * v + shift - v).into(),
        rotation_rad: angles,
    };
    o.rays[0].origin_mm = (rot * V3::from(base.rays[0].origin_mm) + shift).into();
    o.rays[0].direction = (rot * V3::from(base.rays[0].direction)).into();
    let b = trace_spatial(&p, &o, &AtomicBool::new(false)).unwrap();
    assert_eq!(b[0].status, RayStatus::Complete);
    for (x, y) in a[0].points.iter().zip(&b[0].points) {
        assert!((rot * V3::from(*x) + shift - V3::from(*y)).norm() < 1e-8);
    }
    assert!((a[0].optical_path_mm - b[0].optical_path_mm).abs() < 1e-8);
}
#[test]
fn tilted_plane_obeys_analytic_snell_and_plane_intersection() {
    let mut p = project();
    for s in &mut p.surfaces {
        s.curvature = 0.;
        s.conic = 0.;
        s.asphere = [0.; 4];
    }
    let mut o = options();
    o.poses = vec![Pose {
        offset_mm: [0.; 3],
        rotation_rad: [0., 0.1, 0.],
    }];
    let out = trace_spatial(&p, &o, &AtomicBool::new(false)).unwrap();
    let h = &out[0].hits[0];
    let normal = V3::new(0.1_f64.sin(), 0., 0.1_f64.cos());
    let origin = V3::from(o.rays[0].origin_mm);
    let d = V3::from(o.rays[0].direction).normalize();
    let t = -normal.dot(&origin) / normal.dot(&d);
    assert!((origin + t * d - V3::from(h.position_mm)).norm() < 1e-10);
    assert!(
        (h.n_before * h.incidence_rad.sin() - h.n_after * h.refraction_rad.unwrap().sin()).abs()
            < 1e-10
    );
}
#[test]
fn masks_cancel_and_invalid_rays_do_not_produce_success() {
    let p = project();
    let mut o = options();
    for aperture in [
        Aperture::Circle { radius: 0.01 },
        Aperture::Ellipse { x: 0.01, y: 1. },
        Aperture::Rectangle { x: 0.01, y: 1. },
        Aperture::Annulus {
            inner: 2.,
            outer: 3.,
        },
    ] {
        o.apertures = vec![Some(aperture)];
        let r = trace_spatial(&p, &o, &AtomicBool::new(false)).unwrap();
        assert_eq!(r[0].status, RayStatus::Blocked);
        assert_eq!(r[0].points.len(), 2);
        assert_eq!(r[0].surface, Some(0));
        assert!(r[0].hits[0].outgoing.is_none());
    }
    assert!(trace_spatial(&p, &o, &AtomicBool::new(true)).is_err());
    o.rays[0].direction = [0.; 3];
    assert!(trace_spatial(&p, &o, &AtomicBool::new(false)).is_err());
}
#[test]
fn prepared_cache_rebuilds_after_material_and_geometry_edits() {
    let mut p = project();
    let origin = V3::new(0., 1., -10.);
    let d = V3::new(0., 0., 1.);
    let a = PreparedTrace::new(&p, 587.5618).trace(origin, d, true, None);
    p.surfaces
        .iter_mut()
        .find(|s| s.curvature != 0.)
        .unwrap()
        .curvature *= 0.9;
    let b = PreparedTrace::new(&p, 587.5618).trace(origin, d, true, None);
    assert_ne!(a.points.last(), b.points.last());
}

#[test]
fn group_rigid_pose_matches_explicit_surface_transforms_and_rejects_overlap() {
    let p = project();
    let mut group = options();
    let angles = [0.01, 0.02, 0.03];
    let shift = V3::new(0.1, 0.2, 0.3);
    group.groups = vec![GroupPose {
        start: 0,
        end: p.surfaces.len() - 1,
        pose: Pose {
            offset_mm: shift.into(),
            rotation_rad: angles,
        },
    }];
    let rot = nalgebra::Rotation3::from_euler_angles(angles[0], angles[1], angles[2]);
    let mut explicit = options();
    explicit.poses = p
        .vertices()
        .iter()
        .map(|z| {
            let v = V3::new(0., 0., *z);
            Pose {
                offset_mm: (rot * v + shift - v).into(),
                rotation_rad: angles,
            }
        })
        .collect();
    let a = trace_spatial(&p, &group, &AtomicBool::new(false)).unwrap();
    let b = trace_spatial(&p, &explicit, &AtomicBool::new(false)).unwrap();
    assert_eq!(a[0].status, b[0].status);
    for (x, y) in a[0].points.iter().zip(&b[0].points) {
        assert!((V3::from(*x) - V3::from(*y)).norm() < 1e-10);
    }
    group.groups.push(group.groups[0].clone());
    assert!(trace_spatial(&p, &group, &AtomicBool::new(false))
        .unwrap_err()
        .contains("overlap"));
}

#[test]
fn total_internal_reflection_keeps_only_actual_intersections() {
    let mut p = project();
    p.surfaces.truncate(2);
    p.stop = 0;
    p.materials.push(Material {
        name: "test glass".into(),
        n: 1.5,
        sellmeier: None,
        range_nm: [380., 780.],
        source: "analytic test".into(),
    });
    for s in &mut p.surfaces {
        s.curvature = 0.;
        s.conic = 0.;
        s.asphere = [0.; 4];
        s.thickness = 5.;
    }
    p.surfaces[0].material = p.materials.len() - 1;
    p.surfaces[1].material = 0;
    let mut o = options();
    o.rays[0].origin_mm = [0., 0., -10.];
    o.rays[0].direction = [0., 0., 1.];
    o.poses = vec![
        Pose::default(),
        Pose {
            offset_mm: [0.; 3],
            rotation_rad: [0., 1., 0.],
        },
    ];
    let ray = trace_spatial(&p, &o, &AtomicBool::new(false))
        .unwrap()
        .remove(0);
    assert_eq!(ray.status, RayStatus::TotalInternalReflection);
    assert_eq!(ray.surface, Some(1));
    assert_eq!(ray.points.len(), 3);
    assert!(ray.hits[1].outgoing.is_none());
    assert!(ray.detector_local_mm.is_none());
}
