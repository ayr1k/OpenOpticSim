//! Three-dimensional sequential tracing. All exported coordinates are global mm.
//! Poses are absolute offsets from each nominal vertex; R = Rz * Ry * Rx.
use crate::{intersect, refract, Project, RayStatus, V3};
use nalgebra::Rotation3;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Pose {
    #[serde(default)]
    pub offset_mm: [f64; 3],
    #[serde(default)]
    pub rotation_rad: [f64; 3],
}
impl Pose {
    fn rotation(&self) -> Rotation3<f64> {
        Rotation3::from_euler_angles(
            self.rotation_rad[0],
            self.rotation_rad[1],
            self.rotation_rad[2],
        )
    }
    fn valid(&self) -> bool {
        self.offset_mm
            .iter()
            .chain(&self.rotation_rad)
            .all(|x| x.is_finite())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Aperture {
    Circle { radius: f64 },
    Ellipse { x: f64, y: f64 },
    Rectangle { x: f64, y: f64 },
    Annulus { inner: f64, outer: f64 },
}
impl Aperture {
    fn valid(&self) -> bool {
        let positive = |x: f64| x.is_finite() && x > 0.;
        match *self {
            Self::Circle { radius } => positive(radius),
            Self::Ellipse { x, y } | Self::Rectangle { x, y } => positive(x) && positive(y),
            Self::Annulus { inner, outer } => {
                positive(outer) && inner.is_finite() && inner >= 0. && inner < outer
            }
        }
    }
    fn contains(&self, p: V3) -> bool {
        match *self {
            Self::Circle { radius } => p.x.hypot(p.y) <= radius + 1e-8,
            Self::Ellipse { x, y } => (p.x / x).powi(2) + (p.y / y).powi(2) <= 1. + 1e-10,
            Self::Rectangle { x, y } => p.x.abs() <= x + 1e-8 && p.y.abs() <= y + 1e-8,
            Self::Annulus { inner, outer } => {
                p.x.hypot(p.y) >= inner - 1e-8 && p.x.hypot(p.y) <= outer + 1e-8
            }
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputRay {
    pub origin_mm: [f64; 3],
    pub direction: [f64; 3],
    pub wavelength_nm: f64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SpatialOptions {
    #[serde(default)]
    pub poses: Vec<Pose>,
    #[serde(default)]
    pub apertures: Vec<Option<Aperture>>,
    #[serde(default)]
    pub detector: Pose,
    #[serde(default)]
    pub groups: Vec<GroupPose>,
    pub rays: Vec<InputRay>,
    #[serde(default = "one")]
    pub threads: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupPose {
    pub start: usize,
    pub end: usize,
    pub pose: Pose,
}
fn one() -> usize {
    1
}
#[derive(Clone, Debug, Serialize)]
pub struct Hit {
    pub surface: usize,
    pub position_mm: [f64; 3],
    pub local_mm: [f64; 3],
    pub normal: [f64; 3],
    pub incident: [f64; 3],
    pub outgoing: Option<[f64; 3]>,
    pub n_before: f64,
    pub n_after: f64,
    pub incidence_rad: f64,
    pub refraction_rad: Option<f64>,
    pub optical_path_mm: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct DebugRay {
    pub ray_index: usize,
    pub wavelength_nm: f64,
    pub points: Vec<[f64; 3]>,
    pub hits: Vec<Hit>,
    pub status: RayStatus,
    pub surface: Option<usize>,
    pub optical_path_mm: f64,
    pub detector_local_mm: Option<[f64; 3]>,
}
pub fn trace_spatial(
    p: &Project,
    options: &SpatialOptions,
    cancel: &AtomicBool,
) -> Result<Vec<DebugRay>, String> {
    let diagnostics = p.validate_structure();
    if !diagnostics.is_empty() {
        return Err(serde_json::to_string(&diagnostics).unwrap_or_default());
    }
    if p.surfaces.is_empty() || options.rays.is_empty() || options.rays.len() > 100_000 {
        return Err("spatial tracing requires surfaces and 1..100000 explicit rays".into());
    }
    if options.poses.len() > p.surfaces.len()
        || options.apertures.len() > p.surfaces.len()
        || options.poses.iter().any(|x| !x.valid())
        || !options.detector.valid()
        || options.apertures.iter().flatten().any(|x| !x.valid())
    {
        return Err("invalid pose or aperture dimensions".into());
    }
    for ray in &options.rays {
        if ray
            .origin_mm
            .iter()
            .chain(&ray.direction)
            .any(|x| !x.is_finite())
            || V3::from(ray.direction).norm() < 1e-12
            || !V3::from(ray.direction).norm().is_finite()
        {
            return Err("ray origin/direction must be finite and direction nonzero".into());
        }
        for material in &p.materials {
            material.index(ray.wavelength_nm)?;
        }
    }
    let vertices = p.vertices();
    let mut frames: Vec<_> = vertices
        .iter()
        .enumerate()
        .map(|(i, z)| {
            let pose = options.poses.get(i).cloned().unwrap_or_default();
            (
                V3::new(0., 0., *z) + V3::from(pose.offset_mm),
                pose.rotation(),
            )
        })
        .collect();
    let mut assigned = vec![false; p.surfaces.len()];
    for group in &options.groups {
        if group.start > group.end || group.end >= p.surfaces.len() || !group.pose.valid() {
            return Err("invalid spatial group range or pose".into());
        }
        let pivot = V3::new(0., 0., vertices[group.start]);
        let rotation = group.pose.rotation();
        for i in group.start..=group.end {
            if assigned[i] {
                return Err("overlapping spatial groups".into());
            }
            assigned[i] = true;
            frames[i].0 = pivot + rotation * (frames[i].0 - pivot) + V3::from(group.pose.offset_mm);
            frames[i].1 = rotation * frames[i].1;
        }
    }
    let trace_one = |(index, ray): (usize, &InputRay)| -> Result<DebugRay, String> {
        let mut q = V3::from(ray.origin_mm);
        let mut d = V3::from(ray.direction).normalize();
        let mut n = 1.;
        let mut out = DebugRay {
            ray_index: index,
            wavelength_nm: ray.wavelength_nm,
            points: vec![q.into()],
            hits: vec![],
            status: RayStatus::Complete,
            surface: None,
            optical_path_mm: 0.,
            detector_local_mm: None,
        };
        for (i, s) in p.surfaces.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            let (vertex, rotation) = &frames[i];
            let (local, normal, t) = match intersect(
                s,
                0.,
                rotation.inverse() * (q - vertex),
                rotation.inverse() * d,
            ) {
                Ok(v) => v,
                Err(e) => {
                    out.status = e;
                    out.surface = Some(i);
                    return Ok(out);
                }
            };
            q = vertex + rotation * local;
            let normal = rotation * normal;
            out.points.push(q.into());
            out.optical_path_mm += n * t;
            let n2 = p.materials[s.material].index(ray.wavelength_nm)?;
            let aperture = options.apertures.get(i).and_then(|a| a.as_ref());
            // A custom aperture is an additional mask, never enlarges the optical surface.
            let blocked = local.x.hypot(local.y) > s.semi_aperture + 1e-8
                || aperture.is_some_and(|a| !a.contains(local));
            let outgoing = if blocked {
                None
            } else {
                refract(d, normal, n, n2)
            };
            out.hits.push(Hit {
                surface: i,
                position_mm: q.into(),
                local_mm: local.into(),
                normal: normal.into(),
                incident: d.into(),
                outgoing: outgoing.map(Into::into),
                n_before: n,
                n_after: n2,
                incidence_rad: d.dot(&normal).abs().clamp(0., 1.).acos(),
                refraction_rad: outgoing.map(|v| v.dot(&normal).abs().clamp(0., 1.).acos()),
                optical_path_mm: out.optical_path_mm,
            });
            match outgoing {
                Some(v) => d = v,
                None => {
                    out.status = if blocked {
                        RayStatus::Blocked
                    } else {
                        RayStatus::TotalInternalReflection
                    };
                    out.surface = Some(i);
                    return Ok(out);
                }
            }
            n = n2;
        }
        let rotation = options.detector.rotation();
        let vertex = V3::new(0., 0., p.image_z()) + V3::from(options.detector.offset_mm);
        let lp = rotation.inverse() * (q - vertex);
        let ld = rotation.inverse() * d;
        if ld.z.abs() < 1e-12 || -lp.z / ld.z < 0. {
            out.status = RayStatus::Missed;
            return Ok(out);
        }
        let t = -lp.z / ld.z;
        out.points.push((q + d * t).into());
        out.optical_path_mm += n * t;
        out.detector_local_mm = Some((lp + ld * t).into());
        Ok(out)
    };
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    let threads = options
        .threads
        .clamp(1, 64)
        .min(std::thread::available_parallelism().map_or(1, usize::from))
        .min(options.rays.len());
    // Scoped workers preserve input order and own no global thread pool or mutable caches.
    std::thread::scope(|scope| {
        let chunk = options.rays.len().div_ceil(threads);
        let handles: Vec<_> = options
            .rays
            .chunks(chunk)
            .enumerate()
            .map(|(block, rays)| {
                let trace_one = &trace_one;
                scope.spawn(move || {
                    rays.iter()
                        .enumerate()
                        .map(|(i, r)| trace_one((block * chunk + i, r)))
                        .collect::<Result<Vec<_>, String>>()
                })
            })
            .collect();
        let mut result = Vec::with_capacity(options.rays.len());
        for handle in handles {
            result.extend(handle.join().map_err(|_| "spatial worker panicked")??);
        }
        Ok(result)
    })
}
