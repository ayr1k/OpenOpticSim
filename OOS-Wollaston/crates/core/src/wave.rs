//! Scalar reference-sphere diffraction with uniform projected pupil amplitude.
//! See docs/DIFFRACTION.md for conventions and numerical acceptance criteria.
use crate::*;
use rustfft::{num_complex::Complex, FftPlanner};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
#[path = "wave_direct.rs"]
mod direct;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WaveOptions {
    pub method: WaveMethod,
    pub field_index: usize,
    /// None: incoherent sum of positive-weight project wavelengths.
    pub wavelength_nm: Option<f64>,
    pub max_pupil_samples: usize,
}
impl Default for WaveOptions {
    fn default() -> Self {
        Self {
            method: WaveMethod::Fast,
            field_index: 0,
            wavelength_nm: None,
            max_pupil_samples: 256,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaveMethod {
    #[default]
    Fast,
    SphereKirchhoff,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaveBand {
    pub wavelength_nm: f64,
    pub weight: f64,
    pub rms_waves: f64,
    pub chief_image_mm: [f64; 2],
    pub pupil_samples: usize,
    pub blocked_samples: usize,
    pub max_phase_step_waves: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaveQuality {
    pub image_nyquist_ratio: Option<f64>,
    pub converged: bool,
    pub pupil_sampling: usize,
    pub previous_sampling: usize,
    pub mtf_max_change: f64,
    pub psf_l1_change: f64,
    pub edge_energy: f64,
    pub local_expansion_error_waves: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaveResult {
    pub coverage: Option<crate::fields::Coverage>,
    pub method: WaveMethod,
    pub size: usize,
    /// Pixel masses, sum=1. Never peak-normalized or cropped.
    pub psf: Vec<f64>,
    pub mtf_sagittal: Vec<f64>,
    pub mtf_tangential: Vec<f64>,
    pub pixel_um: f64,
    pub frequency_step_lp_mm: f64,
    pub field: f64,
    pub reference_image_mm: [f64; 2],
    pub bands: Vec<WaveBand>,
    pub quality: WaveQuality,
    pub warning: String,
}
/// Inspectable pupil for independent validation. Null means outside aperture.
/// OPD is measured on a reference sphere, not on a paraxial stop plane.
#[derive(Serialize)]
pub struct PupilGrid {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sphere_points_mm: Option<Vec<Option<[f64; 3]>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directions: Option<Vec<Option<[f64; 3]>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amplitude: Option<Vec<Option<f64>>>,
    pub size: usize,
    pub pitch_mm: f64,
    pub centre_mm: [f64; 3],
    pub reference_image_mm: [f64; 3],
    pub reference_radius_mm: f64,
    pub reference_opl_mm: f64,
    pub wavelength_nm: f64,
    pub field: f64,
    pub opd_mm: Vec<Option<f64>>,
    pub launch_xy_mm: Vec<Option<[f64; 2]>>,
    pub blocked_samples: usize,
}
#[derive(Serialize)]
pub struct PupilBundle {
    pub image_pixel_um: f64,
    pub reference_image_mm: [f64; 2],
    pub fft_size: usize,
    pub bands: Vec<(f64, PupilGrid)>,
}
fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Cancelled".into())
    } else {
        Ok(())
    }
}
fn fft2(data: &mut [Complex<f64>], n: usize) {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);
    for row in data.chunks_mut(n) {
        fft.process(row);
    }
    let mut col = vec![Complex::default(); n];
    for x in 0..n {
        for y in 0..n {
            col[y] = data[y * n + x];
        }
        fft.process(&mut col);
        for y in 0..n {
            data[y * n + x] = col[y];
        }
    }
}
pub fn psf_to_mtf(psf: &[f64], n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut otf: Vec<_> = psf.iter().map(|v| Complex::new(*v, 0.)).collect();
    fft2(&mut otf, n);
    let dc = otf[0].norm().max(1e-100);
    (
        (0..=n / 2).map(|i| otf[i].norm() / dc).collect(),
        (0..=n / 2).map(|i| otf[i * n].norm() / dc).collect(),
    )
}
pub fn pupil_to_psf(mut pupil: Vec<Complex<f64>>, n: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    fft2(&mut pupil, n);
    let energy = pupil.iter().map(|c| c.norm_sqr()).sum::<f64>();
    let mut psf = vec![0.; n * n];
    for y in 0..n {
        for x in 0..n {
            psf[((y + n / 2) % n) * n + (x + n / 2) % n] =
                pupil[y * n + x].norm_sqr() / energy.max(1e-100);
        }
    }
    let (sx, sy) = psf_to_mtf(&psf, n);
    (psf, sx, sy)
}
struct Geometry {
    method: WaveMethod,
    image: V3,
    centre: V3,
    radius: f64,
    chief_opl: f64,
    launch: [f64; 2],
    boundary: Vec<[f64; 2]>,
    width: f64,
    nm: f64,
    field: f64,
}
fn on_sphere(
    p: &Project,
    ray: &RayPath,
    image: V3,
    radius: f64,
    field: f64,
) -> Result<(V3, f64), String> {
    if ray.status != RayStatus::Complete {
        return Err(crate::ray_path_error(ray));
    }
    let exit = V3::from(ray.points[ray.points.len() - 2]);
    let d = V3::from(ray.direction);
    let delta = exit - image;
    let b = delta.dot(&d);
    let disc = b * b - delta.norm_squared() + radius * radius;
    if disc <= 0. {
        return Err("Wave reference sphere not intersected; focus first".into());
    }
    let t = -b - disc.sqrt();
    let q = exit + t * d;
    let origin = V3::from(ray.points[0]);
    let incoming = if let Some(dist) = p.object_distance {
        // Reference the incident phase to (0,0,launch_z). Rationalizing the
        // difference avoids subtracting ~1e12 mm paths to recover sub-nm OPD.
        let lateral = dist * field.tan();
        let length = (origin - V3::new(0., -lateral, -dist)).norm();
        let reference = lateral.hypot(dist + origin.z);
        (origin.x * origin.x + origin.y * (origin.y + 2. * lateral)) / (length + reference)
    } else {
        origin.y * field.sin()
    };
    let opl =
        incoming + ray.optical_path - (V3::from(*ray.points.last().unwrap()) - exit).norm() + t;
    Ok((q, opl))
}
fn geometry(
    p: &Project,
    field: f64,
    nm: f64,
    method: WaveMethod,
    cancel: &AtomicBool,
) -> Result<Geometry, String> {
    cancelled(cancel)?;
    let limit = if method == WaveMethod::Fast {
        0.15
    } else {
        crate::fields::MAX_FIELD_RAD
    };
    if !field.is_finite() || !nm.is_finite() || field.abs() > limit || !(380. ..=780.).contains(&nm)
    {
        return Err(format!(
            "Wave applicability: {:?} field limit ±{limit} rad; wavelength 380–780 nm",
            method
        ));
    }
    let px = paraxial(p, nm)?;
    let xp = px.exit_pupil_z.ok_or("Wave exit pupil at infinity")?;
    let n_image = p.materials[p.surfaces.last().unwrap().material].index(nm)?;
    if n_image != 1. || p.image_z() <= xp {
        return Err(
            "Wave applicability: air image space and positive exit-pupil distance required".into(),
        );
    }
    let chief = aimed_ray(p, field, nm, [0., 0.]);
    if chief.status != RayStatus::Complete {
        return Err(crate::ray_path_error(&chief));
    }
    let image = V3::from(*chief.points.last().unwrap());
    let exit = V3::from(chief.points[chief.points.len() - 2]);
    let d = V3::from(chief.direction);
    let centre = exit + (xp - exit.z) / d.z * d;
    let radius = (image - centre).norm();
    if !radius.is_finite() || radius <= 1e-9 || 32. * f64::EPSILON * radius > nm * 1e-6 * 1e-5 {
        return Err("Wave reference sphere is numerically ill-conditioned; exit pupil too distant or degenerate".into());
    }
    let chief_opl = on_sphere(p, &chief, image, radius, field)?.1;
    let mut boundary = Vec::new();
    let mut extent: f64 = 0.;
    for i in 0..256 {
        cancelled(cancel)?;
        let a = std::f64::consts::TAU * i as f64 / 256.;
        let r = p.surfaces[p.stop].semi_aperture;
        let ray = aimed_ray_with_clip(p, field, nm, [r * a.cos(), r * a.sin()], false);
        let q = on_sphere(p, &ray, image, radius, field)?.0;
        let na = (q.x - image.x).hypot(q.y - image.y) / radius;
        let limit = if method == WaveMethod::Fast {
            0.25
        } else {
            0.85
        };
        if na > limit || q.z >= image.z {
            return Err(format!("Wave applicability: {:?} reference-sphere transverse direction sine exceeds {limit}, or pupil crosses sensor hemisphere",method));
        }
        extent = extent
            .max((q.x - centre.x).abs())
            .max((q.y - centre.y).abs());
        boundary.push([q.x, q.y]);
    }
    let mut sign = 0.;
    for i in 0..boundary.len() {
        let a = boundary[i];
        let b = boundary[(i + 1) % boundary.len()];
        let c = boundary[(i + 2) % boundary.len()];
        let cross = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
        if i == 0 {
            sign = cross.signum();
        }
        if cross * sign <= 0. {
            return Err("Wave pupil is folded or non-convex; unsupported mapping".into());
        }
    }
    if extent <= 1e-9 || !extent.is_finite() {
        return Err("Wave degenerate pupil".into());
    }
    Ok(Geometry {
        method,
        image,
        centre,
        radius,
        chief_opl,
        launch: [chief.points[0][0], chief.points[0][1]],
        boundary,
        width: 2.02 * extent,
        nm,
        field,
    })
}
fn inside(poly: &[[f64; 2]], q: [f64; 2]) -> bool {
    let mut sign = 0.;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let cross = (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]);
        if cross.abs() < 1e-14 {
            continue;
        }
        if sign == 0. {
            sign = cross.signum();
        }
        if cross * sign < 0. {
            return false;
        }
    }
    true
}
fn inverse_ray(
    p: &Project,
    prepared: &PreparedTrace<'_>,
    g: &Geometry,
    target: [f64; 2],
) -> Result<(RayPath, f64, f64), String> {
    let mut xy = g.launch;
    let mut orientation = 0.;
    for _ in 0..16 {
        let (q, d) = prepared.launch(g.field, xy);
        let ray = prepared.trace(q, d, false, None);
        let (point, opl) = on_sphere(p, &ray, g.image, g.radius, g.field)?;
        let e = [point.x - target[0], point.y - target[1]];
        if e[0].hypot(e[1]) < 1e-9 && g.method == WaveMethod::Fast {
            return Ok((ray, opl - g.chief_opl, 1.));
        }
        let mut j = [[0.; 2]; 2];
        for k in 0..2 {
            let mut x = xy;
            x[k] += 1e-5;
            let (q, d) = prepared.launch(g.field, x);
            let ray = prepared.trace(q, d, false, None);
            let (probe, _) = on_sphere(p, &ray, g.image, g.radius, g.field)?;
            j[0][k] = (probe.x - point.x) / 1e-5;
            j[1][k] = (probe.y - point.y) / 1e-5;
        }
        let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
        if det.abs() < 1e-10 {
            break;
        }
        if orientation == 0. {
            orientation = det.signum();
        }
        if det * orientation <= 0. {
            return Err("Wave folded pupil mapping".into());
        }
        if e[0].hypot(e[1]) < 1e-9 {
            return Ok((ray, opl - g.chief_opl, 1. / det.abs()));
        }
        xy[0] -= (j[1][1] * e[0] - j[0][1] * e[1]) / det;
        xy[1] -= (-j[1][0] * e[0] + j[0][0] * e[1]) / det;
    }
    Err("Wave pupil inverse mapping did not converge".into())
}
fn sample(p: &Project, g: &Geometry, pitch: f64, cancel: &AtomicBool) -> Result<PupilGrid, String> {
    let size = 2 * ((g.width / pitch / 2.).ceil() as usize) + 1;
    let prepared = PreparedTrace::new(p, g.nm);
    let mut grid = PupilGrid {
        sphere_points_mm: (g.method == WaveMethod::SphereKirchhoff)
            .then(|| vec![None; size * size]),
        directions: (g.method == WaveMethod::SphereKirchhoff).then(|| vec![None; size * size]),
        amplitude: (g.method == WaveMethod::SphereKirchhoff).then(|| vec![None; size * size]),
        size,
        pitch_mm: pitch,
        centre_mm: g.centre.into(),
        reference_image_mm: g.image.into(),
        reference_radius_mm: g.radius,
        reference_opl_mm: g.chief_opl,
        wavelength_nm: g.nm,
        field: g.field,
        opd_mm: vec![None; size * size],
        launch_xy_mm: vec![None; size * size],
        blocked_samples: 0,
    };
    for y in 0..size {
        cancelled(cancel)?;
        for x in 0..size {
            let target = [
                g.centre.x + (x as f64 - (size / 2) as f64) * pitch,
                g.centre.y + (y as f64 - (size / 2) as f64) * pitch,
            ];
            if !inside(&g.boundary, target) {
                continue;
            }
            let (ray, opd, inverse_area) = inverse_ray(p, &prepared, g, target)?;
            if p.surfaces.iter().enumerate().any(|(i, s)| {
                ray.points[i + 1][0].hypot(ray.points[i + 1][1]) > s.semi_aperture + 1e-8
            }) {
                grid.blocked_samples += 1;
                continue;
            }
            grid.opd_mm[y * size + x] = Some(opd);
            grid.launch_xy_mm[y * size + x] = Some([ray.points[0][0], ray.points[0][1]]);
            if g.method == WaveMethod::SphereKirchhoff {
                let q = on_sphere(p, &ray, g.image, g.radius, g.field)?.0;
                let normal = (g.image - q) / g.radius;
                let direction = V3::from(ray.direction);
                if normal.z <= 0. || normal.dot(&direction) <= 0. {
                    return Err("Wave unsupported backward/folded spherical pupil".into());
                }
                grid.sphere_points_mm.as_mut().unwrap()[y * size + x] = Some(q.into());
                grid.directions.as_mut().unwrap()[y * size + x] = Some(ray.direction);
                // Ray-tube energy transport from a uniformly illuminated
                // incident plane (or a uniform point emitter at finite range).
                // |det d(launch_xy)/d(sphere_xy)| includes pupil aberration;
                // dA_sphere=dX dY/n_z. No Fresnel losses or polarization.
                let origin = V3::from(ray.points[0]);
                let incoming = prepared.launch(g.field, [origin.x, origin.y]).1;
                let relative_intensity = if let Some(distance) = p.object_distance {
                    let object = V3::new(0., -distance * g.field.tan(), -distance);
                    let chief_origin = prepared.launch(g.field, g.launch).0;
                    (chief_origin - object).norm_squared() / (origin - object).norm_squared()
                } else {
                    1.
                };
                let amplitude = (relative_intensity * incoming.z * inverse_area * normal.z
                    / normal.dot(&direction))
                .sqrt();
                if !amplitude.is_finite() {
                    return Err("Wave pupil energy transport is ill-conditioned".into());
                }
                grid.amplitude.as_mut().unwrap()[y * size + x] = Some(amplitude);
            }
        }
    }
    if grid.opd_mm.iter().flatten().count() < 32 {
        return Err("Wave insufficient illuminated pupil samples".into());
    }
    Ok(grid)
}
pub fn pupil_grid(
    p: &Project,
    field: f64,
    nm: f64,
    sampling: usize,
    cancel: &AtomicBool,
) -> Result<PupilGrid, String> {
    if !p.validate_optical().is_empty() {
        return Err("Invalid project".into());
    }
    if !(16..=256).contains(&sampling) {
        return Err("Wave pupil sampling must be 16–256".into());
    }
    let g = geometry(p, field, nm, WaveMethod::Fast, cancel)?;
    sample(p, &g, g.width / sampling as f64, cancel)
}
fn calculate(
    p: &Project,
    geometries: &[(Geometry, f64)],
    sampling: usize,
    pixel: f64,
    reference: [f64; 2],
    cancel: &AtomicBool,
) -> Result<WaveResult, String> {
    let n = 4 * sampling;
    let mut psf = vec![0.; n * n];
    let mut bands = Vec::new();
    for (g, weight) in geometries {
        cancelled(cancel)?;
        let lambda = g.nm * 1e-6;
        // Wavelength-specific pupil pitch produces a COMMON physical image
        // grid directly. No intensity resampling; lateral color stays intact.
        let pitch = lambda * g.radius / (n as f64 * pixel);
        let grid = sample(p, g, pitch, cancel)?;
        let mut pupil = vec![Complex::new(0., 0.); n * n];
        let count = grid.opd_mm.iter().flatten().count();
        let mean = grid.opd_mm.iter().flatten().sum::<f64>() / count as f64;
        let rms = (grid
            .opd_mm
            .iter()
            .flatten()
            .map(|v| ((v - mean) / lambda).powi(2))
            .sum::<f64>()
            / count as f64)
            .sqrt();
        let shift = [g.image.x - reference[0], g.image.y - reference[1]];
        let phase_at = |x: usize, y: usize, v: f64| {
            (v - mean) / lambda
                + ((x as f64 - (grid.size / 2) as f64) * pitch * shift[0]
                    + (y as f64 - (grid.size / 2) as f64) * pitch * shift[1])
                    / (lambda * g.radius)
        };
        let mut max_step: f64 = 0.;
        for y in 0..grid.size {
            for x in 0..grid.size {
                if let Some(v) = grid.opd_mm[y * grid.size + x] {
                    let phase = phase_at(x, y, v);
                    for (xx, yy) in [(x + 1, y), (x, y + 1)] {
                        if xx < grid.size && yy < grid.size {
                            if let Some(vv) = grid.opd_mm[yy * grid.size + xx] {
                                max_step = max_step.max((phase - phase_at(xx, yy, vv)).abs());
                            }
                        }
                    }
                    let xx = n / 2 + x - grid.size / 2;
                    let yy = n / 2 + y - grid.size / 2;
                    let amplitude = grid
                        .amplitude
                        .as_ref()
                        .and_then(|a| a[y * grid.size + x])
                        .unwrap_or(1.);
                    pupil[yy * n + xx] =
                        Complex::from_polar(amplitude, std::f64::consts::TAU * phase);
                }
            }
        }
        let mono = pupil_to_psf(pupil, n).0;
        for (a, b) in psf.iter_mut().zip(mono) {
            *a += weight * b;
        }
        bands.push(WaveBand {
            wavelength_nm: g.nm,
            weight: *weight,
            rms_waves: rms,
            chief_image_mm: [g.image.x, g.image.y],
            pupil_samples: count,
            blocked_samples: grid.blocked_samples,
            max_phase_step_waves: max_step,
        });
    }
    let (sx, sy) = psf_to_mtf(&psf, n);
    let mut edge = 0.;
    let mut radial = vec![0.; n];
    for y in 0..n {
        for x in 0..n {
            let r = (x as f64 - n as f64 / 2.).hypot(y as f64 - n as f64 / 2.);
            radial[(r.ceil() as usize).min(n - 1)] += psf[y * n + x];
            if x < n / 10 || y < n / 10 || x >= n - n / 10 || y >= n - n / 10 {
                edge += psf[y * n + x];
            }
        }
    }
    let mut enc = 0.;
    let mut r99 = 0.;
    for (i, v) in radial.into_iter().enumerate() {
        enc += v;
        if enc >= 0.99 {
            r99 = i as f64 * pixel;
            break;
        }
    }
    // Bound local Fourier expansion over the 99% energy radius, after
    // removing a common quadratic image phase. Not a vector-optics bound.
    let expansion = geometries
        .iter()
        .map(|(g, _)| {
            let max_s = g
                .boundary
                .iter()
                .map(|q| (q[0] - g.image.x).hypot(q[1] - g.image.y) / g.radius)
                .fold(0., f64::max);
            let r = r99 + (g.image.x - reference[0]).hypot(g.image.y - reference[1]);
            r * r * max_s * max_s / (2. * g.radius * g.nm * 1e-6)
                + r.powi(3) / (2. * g.radius.powi(2) * g.nm * 1e-6)
        })
        .fold(0., f64::max);
    Ok(WaveResult {
        coverage:None, method:WaveMethod::Fast, size:n, psf, mtf_sagittal:sx, mtf_tangential:sy,
        pixel_um:pixel*1000., frequency_step_lp_mm:1./(n as f64*pixel), field:geometries[0].0.field,
        reference_image_mm:reference, bands,
        quality:WaveQuality {image_nyquist_ratio:None,converged:false,pupil_sampling:sampling,previous_sampling:0,mtf_max_change:0.,psf_l1_change:0.,edge_energy:edge,local_expansion_error_waves:expansion},
        warning:"均匀投影瞳振幅的快速标量衍射模型；已检查采样收敛。未计入镀膜、偏振、Fresnel 透射、相对照度或传感器响应。数值收敛不等于任意镜头的物理精度认证。".into(),
    })
}

fn compare(old: &WaveResult, new: &WaveResult) -> (f64, f64) {
    let ratio = new.size / old.size;
    let mut mtf: f64 = 0.;
    for (a, b) in [
        (&old.mtf_sagittal, &new.mtf_sagittal),
        (&old.mtf_tangential, &new.mtf_tangential),
    ] {
        for (i, v) in a.iter().enumerate() {
            mtf = mtf.max((v - b[i * ratio]).abs());
        }
    }
    let offset = (new.size - old.size) / 2;
    let mut overlap = 0.;
    let mut l1 = 0.;
    for y in 0..old.size {
        for x in 0..old.size {
            let v = new.psf[(y + offset) * new.size + x + offset];
            overlap += v;
            l1 += (old.psf[y * old.size + x] - v).abs();
        }
    }
    (mtf, l1 + (1. - overlap).max(0.))
}
pub fn diffraction(
    p: &Project,
    options: &WaveOptions,
    cancel: &AtomicBool,
) -> Result<WaveResult, String> {
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};
    // Shared by native MTF and test-chart requests. Exact serialized inputs,
    // including all dispersion coefficients and sampling settings; no rounded keys.
    // At most eight entries and 64 MiB of PSF samples are retained.
    static CACHE: OnceLock<Mutex<VecDeque<(String, WaveResult)>>> = OnceLock::new();
    cancelled(cancel)?;
    let key = serde_json::to_string(&(p, options)).map_err(|e| e.to_string())?;
    let cache = CACHE.get_or_init(|| Mutex::new(VecDeque::new()));
    if let Ok(mut entries) = cache.lock() {
        if let Some(i) = entries.iter().position(|(k, _)| *k == key) {
            let entry = entries.remove(i).unwrap();
            let result = entry.1.clone();
            entries.push_back(entry);
            cancelled(cancel)?;
            return Ok(result);
        }
    }
    let result = diffraction_uncached(p, options, cancel)?;
    cancelled(cancel)?;
    if let Ok(mut entries) = cache.lock() {
        entries.retain(|(k, _)| *k != key);
        while !entries.is_empty()
            && (entries.len() >= 8
                || entries.iter().map(|(_, w)| w.psf.len() * 8).sum::<usize>()
                    + result.psf.len() * 8
                    > 64 * 1024 * 1024)
        {
            entries.pop_front();
        }
        if result.psf.len() * 8 <= 64 * 1024 * 1024 {
            entries.push_back((key, result.clone()));
        }
    }
    Ok(result)
}
pub fn diffraction_uncached(
    p: &Project,
    options: &WaveOptions,
    cancel: &AtomicBool,
) -> Result<WaveResult, String> {
    cancelled(cancel)?;
    if !p.validate_optical().is_empty() {
        return Err("Invalid project".into());
    }
    let resolved = crate::fields::resolve(p, cancel)?;
    let p = &resolved;
    if ![128, 256].contains(&options.max_pupil_samples) {
        return Err("Wave maximum pupil samples must be 128 or 256".into());
    }
    let (geometries, reference, pixel) = setup(p, options, cancel)?;
    let calculate = |sampling| {
        if options.method == WaveMethod::SphereKirchhoff {
            direct::calculate_direct(p, &geometries, sampling, pixel, reference, cancel)
        } else {
            calculate(p, &geometries, sampling, pixel, reference, cancel)
        }
    };
    let mut previous = calculate(64)?;
    for sampling in [128, 256] {
        if sampling > options.max_pupil_samples {
            break;
        }
        let mut current = calculate(sampling)?;
        let (mtf, l1) = compare(&previous, &current);
        current.quality.previous_sampling = previous.quality.pupil_sampling;
        current.quality.mtf_max_change = mtf;
        current.quality.psf_l1_change = l1;
        let phase = current
            .bands
            .iter()
            .map(|b| b.max_phase_step_waves)
            .fold(0., f64::max);
        if mtf <= 0.015
            && l1 <= 0.04
            && phase <= 0.25
            && current.quality.edge_energy <= 0.005
            && current.quality.local_expansion_error_waves <= 0.05
        {
            current.quality.converged = true;
            current.coverage = analyze(p, 17, cancel)?.coverage;
            if let Some(c) = current.coverage.as_mut() {
                if let Some(f) = c
                    .fields
                    .iter_mut()
                    .find(|f| (f.angle_rad - current.field).abs() < 1e-12)
                {
                    if options.method == WaveMethod::Fast {
                        f.fast_mtf = "converged".into();
                    } else {
                        f.wide_mtf = "converged".into();
                    }
                }
            }
            cancelled(cancel)?;
            return Ok(current);
        }
        previous = current;
    }
    Err(format!("Wave did not converge: pupil {}, MTF Δ={:.4}, PSF L1={:.4}, phase step={:.3} waves, edge={:.4}, local error={:.3} waves. Focus first, reduce aperture/field, or use 256 samples.",previous.quality.pupil_sampling,previous.quality.mtf_max_change,previous.quality.psf_l1_change,previous.bands.iter().map(|b|b.max_phase_step_waves).fold(0.,f64::max),previous.quality.edge_energy,previous.quality.local_expansion_error_waves))
}
type Setup = (Vec<(Geometry, f64)>, [f64; 2], f64);
fn setup(p: &Project, options: &WaveOptions, cancel: &AtomicBool) -> Result<Setup, String> {
    let field = *p
        .fields
        .get(options.field_index)
        .ok_or("Wave field index out of range")?;
    let wavelengths = if let Some(nm) = options.wavelength_nm {
        vec![(nm, 1.)]
    } else {
        let scale = p.wavelengths.iter().map(|w| w.weight).fold(0., f64::max);
        let sum = p.wavelengths.iter().map(|w| w.weight / scale).sum::<f64>();
        p.wavelengths
            .iter()
            .filter(|w| w.weight > 0.)
            .map(|w| (w.nm, (w.weight / scale) / sum))
            .collect()
    };
    let mut geometries = Vec::new();
    for (nm, weight) in wavelengths {
        geometries.push((geometry(p, field, nm, options.method, cancel)?, weight));
    }
    let ref_g = &geometries
        .iter()
        .min_by(|a, b| {
            (a.0.nm - 587.5618)
                .abs()
                .total_cmp(&(b.0.nm - 587.5618).abs())
        })
        .unwrap()
        .0;
    let reference = [ref_g.image.x, ref_g.image.y];
    let pixel = geometries
        .iter()
        .map(|(g, _)| {
            g.nm * 1e-6 * g.radius
                / (if options.method == WaveMethod::Fast {
                    4.
                } else {
                    2.5
                } * g.width)
        })
        .fold(f64::INFINITY, f64::min);
    Ok((geometries, reference, pixel))
}
/// Export actual multi-band pupil grids for reproducible external validation.
/// This is raw diagnostic data; it does NOT carry a convergence claim.
pub fn pupils(
    p: &Project,
    options: &WaveOptions,
    sampling: usize,
    cancel: &AtomicBool,
) -> Result<PupilBundle, String> {
    let resolved = crate::fields::resolve(p, cancel)?;
    let p = &resolved;
    if !p.validate_optical().is_empty() {
        return Err("Invalid project".into());
    }
    if ![64, 128, 256].contains(&sampling) {
        return Err("Wave diagnostic sampling must be 64, 128 or 256".into());
    }
    let (geometries, reference, pixel) = setup(p, options, cancel)?;
    let mut bands = Vec::new();
    for (g, weight) in geometries {
        bands.push((
            weight,
            sample(
                p,
                &g,
                if options.method == WaveMethod::Fast {
                    g.nm * 1e-6 * g.radius / (4. * sampling as f64 * pixel)
                } else {
                    g.width / sampling as f64
                },
                cancel,
            )?,
        ));
    }
    Ok(PupilBundle {
        image_pixel_um: pixel * 1000.,
        reference_image_mm: reference,
        fft_size: if options.method == WaveMethod::Fast {
            4 * sampling
        } else {
            256
        },
        bands,
    })
}
/// Backwards-compatible monochromatic Rust entry point.
pub fn wave(p: &Project, field: f64, nm: f64, cancel: &AtomicBool) -> Result<WaveResult, String> {
    let mut project = p.clone();
    project.fields = vec![field];
    diffraction(
        &project,
        &WaveOptions {
            wavelength_nm: Some(nm),
            ..WaveOptions::default()
        },
        cancel,
    )
}
