//! Periodically extended planar targets convolved with the complete local PSF.
//! Source and PSF share a physical grid; no truncation or kernel renormalization.
use crate::*;
use rustfft::{num_complex::Complex, FftPlanner};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Serialize)]
pub struct TestFrame {
    pub method: wave::WaveMethod,
    pub field: f64,
    pub size: usize,
    pub computation_size: usize,
    pub width_mm: f64,
    pub source: Vec<u8>,
    pub image: Vec<u8>,
    pub retained_energy: f64,
    pub shift_mm: Option<f64>,
    pub reference_image_mm: [f64; 2],
    pub bands: Vec<wave::WaveBand>,
    pub quality: wave::WaveQuality,
}
#[derive(Serialize)]
pub struct PreviewFailure {
    pub field: f64,
    pub error: String,
}
#[derive(Serialize)]
pub struct TestPreview {
    pub coverage: Option<crate::fields::Coverage>,
    pub frames: Vec<TestFrame>,
    pub failures: Vec<PreviewFailure>,
    pub warning: String,
}

fn transform(
    data: &mut [Complex<f64>],
    n: usize,
    inverse: bool,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let mut planner = FftPlanner::new();
    let fft = if inverse {
        planner.plan_fft_inverse(n)
    } else {
        planner.plan_fft_forward(n)
    };
    for row in data.chunks_mut(n) {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        fft.process(row);
    }
    let mut col = vec![Complex::default(); n];
    for x in 0..n {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        for y in 0..n {
            col[y] = data[y * n + x];
        }
        fft.process(&mut col);
        for y in 0..n {
            data[y * n + x] = col[y];
        }
    }
    Ok(())
}
/// Circular convolution: the input target is explicitly periodically extended.
/// Keeping the complex OTF preserves asymmetric PSF phase and displacement.
pub fn convolve_periodic(
    source: &[f64],
    psf: &[f64],
    n: usize,
    cancel: &AtomicBool,
) -> Result<Vec<f64>, String> {
    if n < 2
        || !n.is_multiple_of(2)
        || source.len() != n * n
        || psf.len() != n * n
        || source.iter().chain(psf).any(|v| !v.is_finite())
        || psf.iter().any(|v| *v < 0.)
        || (psf.iter().sum::<f64>() - 1.).abs() > 1e-8
    {
        return Err("Invalid convolution grid or PSF energy".into());
    }
    let mut a: Vec<_> = source.iter().map(|v| Complex::new(*v, 0.)).collect();
    let mut b = vec![Complex::default(); n * n];
    for y in 0..n {
        for x in 0..n {
            b[y * n + x] = Complex::new(psf[((y + n / 2) % n) * n + (x + n / 2) % n], 0.);
        }
    }
    transform(&mut a, n, false, cancel)?;
    transform(&mut b, n, false, cancel)?;
    for (v, k) in a.iter_mut().zip(b) {
        *v *= k;
    }
    transform(&mut a, n, true, cancel)?;
    Ok(a.into_iter().map(|v| v.re / (n * n) as f64).collect())
}
fn pattern(x: f64, y: f64, kind: &str) -> f64 {
    match kind {
        "points" => {
            let dx = (x * 8.).round() - x * 8.;
            let dy = (y * 8.).round() - y * 8.;
            if dx * dx + dy * dy < 0.018 {
                1.
            } else {
                0.
            }
        }
        "resolution" => {
            let cycles = if y < 1. / 3. {
                8.
            } else if y < 2. / 3. {
                16.
            } else {
                32.
            };
            0.5 + 0.4 * (std::f64::consts::TAU * cycles * x).cos()
        }
        _ => {
            if (x * 8.).rem_euclid(1.) < 0.125 || (y * 8.).rem_euclid(1.) < 0.125 {
                0.1
            } else {
                0.9
            }
        }
    }
}
fn display_pixels(data: &[f64], n: usize, out: usize) -> Vec<u8> {
    let bin = n / out;
    let mut image = vec![0u8; out * out];
    for y in 0..out {
        for x in 0..out {
            let mut v = 0.;
            for j in 0..bin {
                for i in 0..bin {
                    v += data[(y * bin + j) * n + x * bin + i];
                }
            }
            image[y * out + x] = (255. * (v / (bin * bin) as f64).clamp(0., 1.)).round() as u8;
        }
    }
    image
}
pub fn preview_with_options(
    p: &Project,
    kind: &str,
    options: &wave::WaveOptions,
    cancel: &AtomicBool,
) -> Result<TestPreview, String> {
    if !["grid", "points", "resolution"].contains(&kind) {
        return Err("Unknown test pattern".into());
    }
    if !p.validate().is_empty() {
        return Err("Invalid project".into());
    }
    let resolved = crate::fields::resolve(p, cancel)?;
    let p = &resolved;
    let mut frames = Vec::new();
    let mut failures = Vec::new();
    for (i, &field) in p.fields.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let opts = wave::WaveOptions {
            field_index: i,
            ..options.clone()
        };
        let w = match wave::diffraction(p, &opts, cancel) {
            Ok(w) => w,
            Err(error) => {
                if error == "Cancelled" {
                    return Err(error);
                }
                failures.push(PreviewFailure { field, error });
                continue;
            }
        };
        let n = w.size;
        let width = w.pixel_um / 1000. * n as f64;
        let source: Vec<_> = (0..n * n)
            .map(|idx| {
                // Area sample the synthetic target on the native wave grid.
                let x = (idx % n) as f64;
                let y = (idx / n) as f64;
                let mut v = 0.;
                for j in 0..4 {
                    for i in 0..4 {
                        v += pattern(
                            (x + (i as f64 + 0.5) / 4.) / n as f64,
                            (y + (j as f64 + 0.5) / 4.) / n as f64,
                            kind,
                        );
                    }
                }
                v / 16.
            })
            .collect();
        let image = convolve_periodic(&source, &w.psf, n, cancel)?;
        let nm = w
            .bands
            .iter()
            .min_by(|a, b| {
                (a.wavelength_nm - 587.5618)
                    .abs()
                    .total_cmp(&(b.wavelength_nm - 587.5618).abs())
            })
            .unwrap()
            .wavelength_nm;
        let shift = if p.object_distance.is_none() {
            paraxial(p, nm)?
                .efl
                .map(|f| w.reference_image_mm[1] - f * field.tan())
        } else {
            None
        };
        frames.push(TestFrame {
            method: w.method,
            field,
            size: 256,
            computation_size: n,
            width_mm: width,
            source: display_pixels(&source, n, 256),
            image: display_pixels(&image, n, 256),
            retained_energy: w.psf.iter().sum(),
            shift_mm: shift,
            reference_image_mm: w.reference_image_mm,
            bands: w.bands,
            quality: w.quality,
        });
    }
    let mut coverage = analyze(p, 17, cancel)?.coverage;
    if let Some(c) = coverage.as_mut() {
        for frame in &frames {
            if let Some(f) = c.fields.iter_mut().find(|f| f.angle_rad == frame.field) {
                if frame.method == wave::WaveMethod::Fast {
                    f.fast_mtf = "converged".into();
                } else {
                    f.wide_mtf = "converged".into();
                }
            }
        }
        for failure in &failures {
            if let Some(f) = c.fields.iter_mut().find(|f| f.angle_rad == failure.field) {
                if options.method == wave::WaveMethod::Fast {
                    if f.fast_mtf != "outside_model" {
                        f.fast_mtf = "failed".into();
                    }
                } else {
                    f.wide_mtf = "failed".into();
                }
            }
        }
    }
    Ok(TestPreview {coverage,frames,failures,warning:"单一物距、周期延拓的局部平面目标；每块以参考主光线像点为中心。完整多色 PSF 与目标在相同网格卷积，显示时面积平均至 256 像素。使用线性灰度，不模拟传感器、相对照度、整幅畸变或三维景深。不同视场的标尺可能不同。".into()})
}
pub fn preview(p: &Project, kind: &str, cancel: &AtomicBool) -> Result<TestPreview, String> {
    preview_with_options(p, kind, &wave::WaveOptions::default(), cancel)
}
