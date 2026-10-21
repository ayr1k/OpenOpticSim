use crate::model::*;
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
pub type V3 = Vector3<f64>;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RayStatus {
    Complete,
    Missed,
    Blocked,
    TotalInternalReflection,
    NonConvergent,
    InvalidMaterial,
    AimingFailed,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TraceStage {
    Aiming,
    Trace,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AimInfo {
    pub target_mm: [f64; 2],
    pub residual_mm: Option<f64>,
    pub iterations: usize,
    pub method: String,
    pub failure: Option<String>,
    pub trial_surface: Option<usize>,
    pub trial_status: Option<RayStatus>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RayPath {
    pub points: Vec<[f64; 3]>,
    pub direction: [f64; 3],
    pub optical_path: f64,
    pub wavelength_nm: f64,
    pub status: RayStatus,
    pub surface: Option<usize>,
    pub stage: TraceStage,
    pub aiming: Option<AimInfo>,
}
pub fn refract(d: V3, mut normal: V3, n1: f64, n2: f64) -> Option<V3> {
    if d.dot(&normal) > 0. {
        normal = -normal;
    }
    let c = -d.dot(&normal);
    let eta = n1 / n2;
    let k = 1. - eta * eta * (1. - c * c);
    if k < -1e-14 {
        None
    } else {
        Some((eta * d + (eta * c - k.max(0.).sqrt()) * normal).normalize())
    }
}
pub fn intersect(s: &Surface, z: f64, p: V3, d: V3) -> Result<(V3, V3, f64), RayStatus> {
    if d.z.abs() < 1e-12 {
        return Err(RayStatus::Missed);
    }
    let mut t = (z - p.z) / d.z;
    // Safeguarded step length; solve z_ray - z_vertex - sag(r_ray) = 0.
    for _ in 0..60 {
        let q = p + t * d;
        let r = q.x.hypot(q.y);
        let (sag, slope) = s.sag(r).ok_or(RayStatus::Missed)?;
        let f = q.z - z - sag;
        let nx = if r > 1e-15 { -slope * q.x / r } else { 0. };
        let ny = if r > 1e-15 { -slope * q.y / r } else { 0. };
        let normal = V3::new(nx, ny, 1.);
        if f.abs() < 1e-10 {
            if t < -1e-8 {
                return Err(RayStatus::Missed);
            }
            return Ok((q, normal.normalize(), t.max(0.)));
        }
        let df = normal.dot(&d);
        if df.abs() < 1e-12 {
            return Err(RayStatus::NonConvergent);
        }
        let step = (f / df).clamp(-100., 100.);
        t -= step;
        if !t.is_finite() {
            return Err(RayStatus::NonConvergent);
        }
    }
    Err(RayStatus::NonConvergent)
}
pub fn trace_ray(
    p: &Project,
    origin: V3,
    direction: V3,
    nm: f64,
    clip: bool,
    until: Option<usize>,
) -> RayPath {
    PreparedTrace::new(p, nm).trace(origin, direction, clip, until)
}
/// Immutable per-wavelength preparation. Rebuild after any prescription edit.
pub struct PreparedTrace<'a> {
    p: &'a Project,
    nm: f64,
    vertices: Vec<f64>,
    indices: Vec<Option<f64>>,
    launch_z: f64,
}
impl<'a> PreparedTrace<'a> {
    pub fn new(p: &'a Project, nm: f64) -> Self {
        let launch_z = p
            .surfaces
            .first()
            .map(|s| {
                (0..=32)
                    .filter_map(|i| s.sag(s.semi_aperture * i as f64 / 32.).map(|v| v.0))
                    .fold(0., f64::min)
                    - 0.5
            })
            .unwrap_or(-0.5);
        Self {
            p,
            nm,
            vertices: p.vertices(),
            indices: p
                .surfaces
                .iter()
                .map(|s| p.materials.get(s.material).and_then(|m| m.index(nm).ok()))
                .collect(),
            launch_z,
        }
    }
    pub fn launch(&self, field: f64, xy: [f64; 2]) -> (V3, V3) {
        let q = V3::new(xy[0], xy[1], self.launch_z);
        let d = if let Some(dist) = self.p.object_distance {
            (q - V3::new(0., -dist * field.tan(), -dist)).normalize()
        } else {
            V3::new(0., field.sin(), field.cos())
        };
        (q, d)
    }
    pub fn trace(&self, origin: V3, direction: V3, clip: bool, until: Option<usize>) -> RayPath {
        let p = self.p;
        let nm = self.nm;
        let mut q = origin;
        let mut d = direction.normalize();
        let mut n = 1.;
        let mut out = RayPath {
            points: vec![q.into()],
            direction: d.into(),
            optical_path: 0.,
            wavelength_nm: nm,
            status: RayStatus::Complete,
            surface: None,
            stage: TraceStage::Trace,
            aiming: None,
        };
        for (i, (s, &z)) in p.surfaces.iter().zip(&self.vertices).enumerate() {
            let (hit, normal, t) = match intersect(s, z, q, d) {
                Ok(v) => v,
                Err(e) => {
                    out.status = e;
                    out.surface = Some(i);
                    return out;
                }
            };
            out.optical_path += n * t;
            out.points.push(hit.into());
            q = hit;
            if clip && q.x.hypot(q.y) > s.semi_aperture + 1e-8 {
                out.status = RayStatus::Blocked;
                out.surface = Some(i);
                return out;
            }
            if until == Some(i) {
                out.direction = d.into();
                return out;
            }
            let n2 = match self.indices[i] {
                Some(v) => v,
                None => {
                    out.status = RayStatus::InvalidMaterial;
                    out.surface = Some(i);
                    return out;
                }
            };
            d = match refract(d, normal, n, n2) {
                Some(v) => v,
                None => {
                    out.status = RayStatus::TotalInternalReflection;
                    out.surface = Some(i);
                    return out;
                }
            };
            n = n2;
            out.direction = d.into();
        }
        if d.z <= 1e-12 {
            out.status = RayStatus::Missed;
            return out;
        }
        let t = (p.image_z() - q.z) / d.z;
        if t < 0. {
            out.status = RayStatus::Missed;
            return out;
        }
        q += t * d;
        out.optical_path += n * t;
        out.points.push(q.into());
        out.direction = d.into();
        out
    }
}
pub fn launch(p: &Project, field: f64, xy: [f64; 2]) -> (V3, V3) {
    let s = &p.surfaces[0];
    let min_sag = (0..=32)
        .filter_map(|i| s.sag(s.semi_aperture * i as f64 / 32.).map(|v| v.0))
        .fold(0., f64::min);
    let q = V3::new(xy[0], xy[1], min_sag - 0.5);
    let d = if let Some(dist) = p.object_distance {
        (q - V3::new(0., -dist * field.tan(), -dist)).normalize()
    } else {
        V3::new(0., field.sin(), field.cos())
    };
    (q, d)
}
/// Newton shooting onto an actual stop coordinate. Upstream apertures are ignored
/// only while aiming; final trace applies every physical aperture.
pub fn aimed_ray(p: &Project, field: f64, nm: f64, target: [f64; 2]) -> RayPath {
    aimed_ray_with_clip(p, field, nm, target, true)
}
/// Aperture-free aiming is used only to locate the pupil boundary. Analysis
/// must explicitly mask every physical surface after this operation.
pub fn aimed_ray_with_clip(
    p: &Project,
    field: f64,
    nm: f64,
    target: [f64; 2],
    clip: bool,
) -> RayPath {
    crate::aiming::PupilAimer::new(p, field, nm).aim(target, clip)
}
