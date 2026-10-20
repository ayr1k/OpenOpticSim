//! Direct scalar Kirchhoff propagation from the traced spherical pupil to the
//! final z=constant detector. No paraxial/Fourier distance expansion.
//! Scalar ray-tube amplitude transport; no interface reflection/polarization.
use super::*;

pub(super) fn calculate_direct(
    p: &Project,
    geometries: &[(Geometry, f64)],
    sampling: usize,
    pixel: f64,
    reference: [f64; 2],
    cancel: &AtomicBool,
) -> Result<WaveResult, String> {
    let n = 256;
    let mut psf = vec![0.; n * n];
    let mut bands = Vec::new();
    let mut nyquist_ratio: f64 = 0.;
    for (g, weight) in geometries {
        cancelled(cancel)?;
        let grid = sample(p, g, g.width / sampling as f64, cancel)?;
        let lambda = g.nm * 1e-6;
        let spread = grid
            .sphere_points_mm
            .as_ref()
            .unwrap()
            .iter()
            .flatten()
            .map(|q| (V3::from(*q) - g.centre).norm())
            .fold(0., f64::max);
        let max_delta = (reference[0] - g.image.x)
            .abs()
            .hypot((reference[1] - g.image.y).abs())
            + (n as f64 / 2.) * pixel * 2f64.sqrt();
        let min_distance = g.radius - spread - max_delta;
        // The sampled source points and their connecting chords lie inside
        // this ball. The direction map has Lipschitz bound 1/min_distance;
        // twice its radius bounds the pupil's spatial-frequency diameter.
        let ratio = 4. * pixel * spread / (lambda * min_distance);
        if min_distance <= 0. || ratio >= 1. {
            return Err("Wave direct image grid violates the conservative Nyquist bound; reduce aperture, aberration or lateral color".into());
        }
        nyquist_ratio = nyquist_ratio.max(ratio);
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
        let mut max_step: f64 = 0.;
        for y in 0..grid.size {
            for x in 0..grid.size {
                if let Some(v) = grid.opd_mm[y * grid.size + x] {
                    for (xx, yy) in [(x + 1, y), (x, y + 1)] {
                        if xx < grid.size && yy < grid.size {
                            if let Some(vv) = grid.opd_mm[yy * grid.size + xx] {
                                max_step = max_step.max((v - vv).abs() / lambda);
                            }
                        }
                    }
                }
            }
        }
        let data: Vec<_> = grid
            .opd_mm
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                v.map(|v| {
                    let q = V3::from(grid.sphere_points_mm.as_ref().unwrap()[i].unwrap());
                    let normal = (g.image - q) / g.radius;
                    let d = V3::from(grid.directions.as_ref().unwrap()[i].unwrap());
                    (
                        normal,
                        normal.dot(&d),
                        (v - mean) / lambda,
                        grid.amplitude.as_ref().unwrap()[i].unwrap(),
                    )
                })
            })
            .collect();
        let mut mono = vec![0.; n * n];
        let workers = std::thread::available_parallelism()
            .map_or(1, usize::from)
            .min(n);
        let rows = n.div_ceil(workers);
        std::thread::scope(|scope| {
            for (chunk, output) in mono.chunks_mut(rows * n).enumerate() {
                let data = &data;
                scope.spawn(move || {
                    for (i, v) in output.iter_mut().enumerate() {
                        if i % n == 0 && cancel.load(Ordering::Relaxed) {
                            return;
                        }
                        let x = i % n;
                        let y = chunk * rows + i / n;
                        let delta = V3::new(
                            reference[0] - g.image.x + (x as f64 - n as f64 / 2.) * pixel,
                            reference[1] - g.image.y + (y as f64 - n as f64 / 2.) * pixel,
                            0.,
                        );
                        let delta2 = delta.norm_squared();
                        // Subtract a common carrier analytically before sin/cos;
                        // retain sub-nanometre path differences in double precision.
                        let chief = (g.image - g.centre) / g.radius;
                        let chief_path = (2. * g.radius * chief.dot(&delta) + delta2)
                            / ((g.radius * g.radius + 2. * g.radius * chief.dot(&delta) + delta2)
                                .sqrt()
                                + g.radius);
                        let mut sum = Complex::new(0., 0.);
                        for &(normal, nd, phase, sphere_amplitude) in data {
                            let dot = normal.dot(&delta);
                            let length =
                                (g.radius * g.radius + 2. * g.radius * dot + delta2).sqrt();
                            let path = (2. * g.radius * dot + delta2) / (length + g.radius);
                            let obliquity = (nd + (g.radius + dot) / length) / 2.;
                            let amplitude =
                                sphere_amplitude * obliquity / normal.z * g.radius / length;
                            sum += Complex::from_polar(
                                amplitude,
                                std::f64::consts::TAU * (phase + (path - chief_path) / lambda),
                            );
                        }
                        *v = sum.norm_sqr();
                    }
                });
            }
        });
        cancelled(cancel)?;
        let energy = mono.iter().sum::<f64>();
        if !energy.is_finite() || energy <= 0. {
            return Err("Wave direct propagation has no finite energy".into());
        }
        for (a, b) in psf.iter_mut().zip(mono) {
            *a += weight * b / energy;
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
    for y in 0..n {
        for x in 0..n {
            if x < n / 10 || y < n / 10 || x >= n - n / 10 || y >= n - n / 10 {
                edge += psf[y * n + x];
            }
        }
    }
    Ok(WaveResult{coverage:None,method:WaveMethod::SphereKirchhoff,size:n,psf,mtf_sagittal:sx,mtf_tangential:sy,pixel_um:pixel*1000.,frequency_step_lp_mm:1./(n as f64*pixel),field:geometries[0].0.field,reference_image_mm:reference,bands,quality:WaveQuality{image_nyquist_ratio:Some(nyquist_ratio),converged:false,pupil_sampling:sampling,previous_sampling:0,mtf_max_change:0.,psf_l1_change:0.,edge_energy:edge,local_expansion_error_waves:0.},warning:"直接标量 Kirchhoff 球面传播；实际平面传感器坐标与尺度。均匀入射光或有限物距均匀点源，按光线管雅可比传输瞳面振幅；包含球面积、实际出射方向与倾斜因子；不使用小视场传播展开。256×256 像面网格，瞳采样逐级收敛；窗口边缘能量未通过时拒绝结果。未计入 Fresnel 透射、镀膜、偏振、实际照度分布、传感器响应。独立验证仅覆盖公开算例，不能认证任意镜头。".into()})
}
