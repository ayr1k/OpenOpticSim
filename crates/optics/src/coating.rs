//! Lossless isotropic dielectric stacks, characteristic admittance matrices.
//! Meridional ray throughput only: a common s/p basis exists along each ray.
use crate::*;
use rustfft::num_complex::Complex64 as C;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub index: f64,
    pub thickness_nm: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stack {
    pub surface: usize,
    pub layers: Vec<Layer>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoatingOptions {
    pub stacks: Vec<Stack>,
}
#[derive(Serialize)]
pub struct Response {
    pub rs: f64,
    pub rp: f64,
    pub ts: f64,
    pub tp: f64,
}
/// Real positive indices only; evanescent layers and absorption are explicit errors.
pub fn response(
    n0: f64,
    ns: f64,
    angle: f64,
    nm: f64,
    layers: &[Layer],
) -> Result<Response, String> {
    if ![n0, ns, angle, nm].iter().all(|v| v.is_finite())
        || n0 < 1.
        || ns < 1.
        || !(0. ..std::f64::consts::FRAC_PI_2).contains(&angle)
        || nm <= 0.
        || layers.len() > 32
        || layers.iter().any(|l| {
            !l.index.is_finite()
                || l.index < 1.
                || l.index > 10.
                || !l.thickness_nm.is_finite()
                || !(0. ..=10000.).contains(&l.thickness_nm)
        })
    {
        return Err("Invalid dielectric coating".into());
    }
    let invariant = n0 * angle.sin();
    let cos = |n: f64| -> Result<f64, String> {
        let q = 1. - (invariant / n).powi(2);
        if q <= 0. {
            Err(
                "Coating model does not support evanescent layers / total internal reflection"
                    .into(),
            )
        } else {
            Ok(q.sqrt())
        }
    };
    let c0 = cos(n0)?;
    let cs = cos(ns)?;
    let mut values = vec![];
    for p in [false, true] {
        let adm = |n: f64, c: f64| if p { n / c } else { n * c };
        let y0 = adm(n0, c0);
        let ys = adm(ns, cs);
        let mut m = [
            C::new(1., 0.),
            C::new(0., 0.),
            C::new(0., 0.),
            C::new(1., 0.),
        ];
        for l in layers {
            let c = cos(l.index)?;
            let y = adm(l.index, c);
            let delta = std::f64::consts::TAU * l.index * l.thickness_nm * c / nm;
            let a = C::new(delta.cos(), 0.);
            let b = C::new(0., delta.sin() / y);
            let d = C::new(0., delta.sin() * y);
            m = [
                m[0] * a + m[1] * d,
                m[0] * b + m[1] * a,
                m[2] * a + m[3] * d,
                m[2] * b + m[3] * a,
            ];
        }
        let b = m[0] + m[1] * ys;
        let c = m[2] + m[3] * ys;
        let den = b * y0 + c;
        let r = ((b * y0 - c) / den).norm_sqr();
        let t = (C::new(2. * y0, 0.) / den).norm_sqr() * ys / y0;
        if !r.is_finite() || !t.is_finite() || (r + t - 1.).abs() > 1e-8 {
            return Err("Coating energy balance failed".into());
        }
        values.push((r, t));
    }
    Ok(Response {
        rs: values[0].0,
        ts: values[0].1,
        rp: values[1].0,
        tp: values[1].1,
    })
}
pub fn analyze_coatings(
    p: &Project,
    o: &CoatingOptions,
    cancel: &AtomicBool,
) -> Result<serde_json::Value, String> {
    if o.stacks.len() > p.surfaces.len() {
        return Err("Too many surface stacks".into());
    }
    for (i, s) in o.stacks.iter().enumerate() {
        if s.surface >= p.surfaces.len() || o.stacks[..i].iter().any(|a| a.surface == s.surface) {
            return Err("Invalid or duplicate coating surface".into());
        }
        response(1., 1.5, 0., 550., &s.layers)?;
    }
    let p = fields::resolve(p, cancel)?;
    let mut rays = vec![];
    for &field in &p.fields {
        for w in &p.wavelengths {
            let mut aimer = aiming::PupilAimer::new(&p, field, w.nm);
            for pupil in [-0.8, 0., 0.8] {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                let ray = aimer.aim([0., pupil * p.surfaces[p.stop].semi_aperture], true);
                if ray.status != RayStatus::Complete {
                    rays.push(serde_json::json!({"field":field,"nm":w.nm,"pupil":pupil,
                        "transmission":null,"status":ray.status}));
                    continue;
                }
                let mut ts = 1.;
                let mut tp = 1.;
                let mut bare_s = 1.;
                let mut bare_p = 1.;
                let mut n = 1.;
                let mut surfaces = vec![];
                let mut error = None;
                for (i, s) in p.surfaces.iter().enumerate() {
                    let d = (V3::from(ray.points[i + 1]) - V3::from(ray.points[i])).normalize();
                    let q = V3::from(ray.points[i + 1]);
                    let radius = q.x.hypot(q.y);
                    let slope = s.sag(radius).ok_or("Invalid sag")?.1;
                    let normal = if radius < 1e-15 {
                        V3::new(0., 0., 1.)
                    } else {
                        V3::new(-slope * q.x / radius, -slope * q.y / radius, 1.).normalize()
                    };
                    let angle = d.dot(&normal).abs().clamp(0., 1.).acos();
                    let n2 = p.materials[s.material].index(w.nm)?;
                    let layers = o
                        .stacks
                        .iter()
                        .find(|v| v.surface == i)
                        .map_or(&[][..], |v| v.layers.as_slice());
                    match (
                        response(n, n2, angle, w.nm, layers),
                        response(n, n2, angle, w.nm, &[]),
                    ) {
                        (Ok(a), Ok(b)) => {
                            ts *= a.ts;
                            tp *= a.tp;
                            bare_s *= b.ts;
                            bare_p *= b.tp;
                            surfaces.push(serde_json::json!({"surface":i,"angle_deg":angle.to_degrees(),"response":a}));
                        }
                        (Err(e), _) | (_, Err(e)) => {
                            error = Some(e);
                            break;
                        }
                    }
                    n = n2;
                }
                rays.push(serde_json::json!({"field":field,"nm":w.nm,"pupil":pupil,
                    "transmission":if error.is_none(){Some((ts+tp)/2.)}else{None},
                    "uncoated":if error.is_none(){Some((bare_s+bare_p)/2.)}else{None},
                    "status":error.unwrap_or("complete".into()),"surfaces":surfaces}));
            }
        }
    }
    Ok(
        serde_json::json!({"rays":rays,"note":"无吸收、常折射率介质膜层；实际子午光线入射角、S/P 分别传递后取平均。仅直透路径，不包含鬼影、散射、整体照度或镀膜相位对 PSF 的影响。"}),
    )
}
