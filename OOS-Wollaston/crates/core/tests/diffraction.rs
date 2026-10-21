use rustfft::num_complex::Complex;
use std::sync::atomic::AtomicBool;
use wollaston::wave::{diffraction, WaveOptions};
use wollaston::*;
fn flag() -> AtomicBool {
    AtomicBool::new(false)
}
fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} != {b}, tol {tol}");
}
fn focused() -> Project {
    let mut p = examples().remove(1);
    p.fields = vec![0., 0.05];
    autofocus(&p, &flag()).unwrap().project
}
#[test]
fn phase_ramp_has_correct_image_direction_and_invariant_mtf() {
    let n = 128;
    let shift = 7.;
    let pupil = |tilt: f64| {
        (0..n * n)
            .map(|i| {
                let x = (i % n) as f64 - n as f64 / 2.;
                let y = (i / n) as f64 - n as f64 / 2.;
                if x * x + y * y <= 16f64.powi(2) {
                    Complex::from_polar(1., std::f64::consts::TAU * tilt * x / n as f64)
                } else {
                    Complex::default()
                }
            })
            .collect()
    };
    let (base, mtf, _) = wave::pupil_to_psf(pupil(0.), n);
    let (tilted, shift_mtf, _) = wave::pupil_to_psf(pupil(shift), n);
    let peak = tilted
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap()
        .0;
    assert_eq!(peak % n, n / 2 + shift as usize);
    for y in 0..n {
        for x in 0..n {
            close(
                tilted[y * n + (x + shift as usize) % n],
                base[y * n + x],
                1e-12,
            );
        }
    }
    for (a, b) in mtf.iter().zip(shift_mtf) {
        close(*a, b, 1e-12);
    }
}
#[test]
fn airy_psf_scale_matches_bessel_reference() {
    // Independent SciPy J1 reference at u=pi*k/4; n=512, radius=64.
    let reference = [
        1.,
        0.8553479588447046,
        0.5208549963417163,
        0.2018104245814295,
        0.032830452075419514,
        0.00037167414446505055,
        0.014289651218299613,
        0.015438933433735544,
        0.004570227665524388,
    ];
    let n = 512;
    let pupil = (0..n * n)
        .map(|i| {
            let x = (i % n) as f64 - 256.;
            let y = (i / n) as f64 - 256.;
            Complex::new(if x * x + y * y <= 64. * 64. { 1. } else { 0. }, 0.)
        })
        .collect();
    let psf = wave::pupil_to_psf(pupil, n).0;
    let peak = psf[256 * n + 256];
    for (k, v) in reference.iter().enumerate() {
        close(psf[256 * n + 256 + k] / peak, *v, 0.001);
    }
}
#[test]
fn incoherent_mix_preserves_otf_phase_not_average_mtf() {
    let n = 64;
    let mut a = vec![0.; n * n];
    let mut b = a.clone();
    a[n / 2 * n + n / 2] = 1.;
    b[n / 2 * n + n / 2 + 4] = 1.;
    let mixed: Vec<_> = a.iter().zip(&b).map(|(a, b)| 0.5 * (a + b)).collect();
    let (ma, _) = wave::psf_to_mtf(&a, n);
    let (mb, _) = wave::psf_to_mtf(&b, n);
    let (mm, _) = wave::psf_to_mtf(&mixed, n);
    close(ma[8], 1., 1e-12);
    close(mb[8], 1., 1e-12);
    close(mm[8], 0., 1e-12);
}
#[test]
fn convolution_energy_constant_shift_and_sinusoid_transfer() {
    let n = 64;
    let mut psf = vec![0.; n * n];
    psf[n / 2 * n + n / 2] = 0.6;
    psf[(n / 2 + 2) * n + n / 2 + 3] = 0.4;
    let constant = vec![0.37; n * n];
    let conv = preview::convolve_periodic(&constant, &psf, n, &flag()).unwrap();
    for v in conv {
        close(v, 0.37, 1e-12);
    }
    let mut delta = vec![0.; n * n];
    delta[0] = 1.;
    let conv = preview::convolve_periodic(&delta, &psf, n, &flag()).unwrap();
    close(conv[0], 0.6, 1e-12);
    close(conv[2 * n + 3], 0.4, 1e-12);
    close(conv.iter().sum(), 1., 1e-12);
    let freq = 5.;
    let source: Vec<_> = (0..n * n)
        .map(|i| 0.5 + 0.4 * (std::f64::consts::TAU * freq * (i % n) as f64 / n as f64).cos())
        .collect();
    let conv = preview::convolve_periodic(&source, &psf, n, &flag()).unwrap();
    let coeff = |phase: f64| {
        (0..n)
            .map(|x| conv[x] * (std::f64::consts::TAU * freq * x as f64 / n as f64 + phase).cos())
            .sum::<f64>()
            * 2.
            / n as f64
    };
    let mtf = wave::psf_to_mtf(&psf, n).0;
    close(
        coeff(0.).hypot(coeff(std::f64::consts::FRAC_PI_2)) / 0.4,
        mtf[5],
        1e-12,
    );
    for i in 0..n * n {
        let y = i / n;
        let x = i % n;
        let exact = 0.6 * source[i] + 0.4 * source[((y + n - 2) % n) * n + (x + n - 3) % n];
        close(conv[i], exact, 1e-12);
    }
    assert!(preview::convolve_periodic(&source, &psf, n, &AtomicBool::new(true)).is_err());
}
#[test]
fn invalid_convolution_never_silently_renormalizes() {
    let n = 8;
    assert!(preview::convolve_periodic(&vec![1.; n * n], &vec![0.; n * n], n, &flag()).is_err());
}
#[test]
fn distant_finite_object_retains_subwavelength_precision() {
    let mut p = focused();
    let a = wave::pupil_grid(&p, 0.05, 587.5618, 64, &flag()).unwrap();
    p.object_distance = Some(1e12);
    let b = wave::pupil_grid(&p, 0.05, 587.5618, 64, &flag()).unwrap();
    assert_eq!(a.size, b.size);
    let mut count = 0;
    for (x, y) in a.opd_mm.iter().zip(&b.opd_mm) {
        if let (Some(x), Some(y)) = (x, y) {
            close(*x, *y, 1e-8);
            count += 1;
        }
    }
    assert!(count > 1000);
}
#[test]
fn spectral_weights_are_scale_invariant_and_duplicate_bands_match_mono() {
    let mut p = focused();
    p.wavelengths = vec![Wavelength {
        nm: 587.5618,
        weight: 1.,
    }];
    let a = diffraction(&p, &WaveOptions::default(), &flag()).unwrap();
    p.wavelengths = vec![
        Wavelength {
            nm: 587.5618,
            weight: 1e308,
        },
        Wavelength {
            nm: 587.5618,
            weight: 1e308,
        },
    ];
    let b = diffraction(&p, &WaveOptions::default(), &flag()).unwrap();
    assert_eq!(a.size, b.size);
    for (x, y) in a.psf.iter().zip(&b.psf) {
        close(*x, *y, 1e-12);
    }
    close(b.bands[0].weight, 0.5, 1e-15);
}
#[test]
fn finite_asphere_and_clipped_pupil_are_supported() {
    let mut p = focused();
    p.object_distance = Some(1000.);
    p.surfaces[1].conic = -0.5;
    p.surfaces[1].asphere[0] = 1e-6;
    p = autofocus(&p, &flag()).unwrap().project;
    let g = wave::pupil_grid(&p, 0.03, 587.5618, 32, &flag()).unwrap();
    assert!(g.opd_mm.iter().flatten().all(|v| v.is_finite()));
    let first_glass = if p.stop == 0 { 1 } else { 0 };
    p.surfaces[first_glass].semi_aperture = p.surfaces[p.stop].semi_aperture * 0.6;
    let g = wave::pupil_grid(&p, 0., 587.5618, 32, &flag()).unwrap();
    assert!(g.blocked_samples > 0);
    assert!(g.opd_mm.iter().flatten().count() > 32);
}
#[test]
fn untrusted_or_out_of_range_wave_results_are_rejected() {
    let mut p = examples().remove(0);
    p.surfaces.last_mut().unwrap().thickness = 100.;
    assert!(diffraction(&p, &WaveOptions::default(), &flag())
        .unwrap_err()
        .contains("did not converge"));
    let opts = WaveOptions {
        max_pupil_samples: 64,
        ..Default::default()
    };
    assert!(diffraction(&p, &opts, &flag()).is_err());
    let opts = WaveOptions {
        field_index: 100,
        ..Default::default()
    };
    assert!(diffraction(&p, &opts, &flag()).is_err());
    let opts = WaveOptions {
        wavelength_nm: Some(f64::NAN),
        ..Default::default()
    };
    assert!(diffraction(&p, &opts, &flag()).is_err());
    assert!(
        diffraction(&p, &WaveOptions::default(), &AtomicBool::new(true))
            .unwrap_err()
            .contains("Cancelled")
    );
}
#[test]
fn preview_reports_each_failed_field_without_losing_valid_frames() {
    let mut p = focused();
    p.fields = vec![0., 0.3];
    let r = preview::preview(&p, "resolution", &flag()).unwrap();
    assert_eq!(r.frames.len(), 1);
    assert_eq!(r.failures.len(), 1);
    assert_eq!(r.failures[0].field, 0.3);
    assert_eq!(r.frames[0].bands.len(), 3);
}
