use openoptics::*;
use std::{sync::atomic::AtomicBool, time::Instant};
fn main() {
    let cancel = AtomicBool::new(false);
    let p = autofocus(&examples().remove(1), &cancel).unwrap().project;
    let options = wave::WaveOptions::default();
    let t = Instant::now();
    let a = wave::diffraction_uncached(&p, &options, &cancel).unwrap();
    let cold = t.elapsed().as_secs_f64() * 1000.;
    wave::diffraction(&p, &options, &cancel).unwrap();
    let t = Instant::now();
    let b = wave::diffraction(&p, &options, &cancel).unwrap();
    let warm = t.elapsed().as_secs_f64() * 1000.;
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(&b).unwrap()
    );
    let t = Instant::now();
    let first = preview::preview(&p, "grid", &cancel).unwrap();
    let first_ms = t.elapsed().as_secs_f64() * 1000.;
    let t = Instant::now();
    let second = preview::preview(&p, "grid", &cancel).unwrap();
    let second_ms = t.elapsed().as_secs_f64() * 1000.;
    assert_eq!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(second).unwrap()
    );
    println!(
        "{}",
        serde_json::json!({"version":"1.3.0","logical_cpus":std::thread::available_parallelism().unwrap().get(),
        "model":p.name,"cold_wave_ms":cold,"cached_wave_ms":warm,"first_preview_ms":first_ms,
        "cached_preview_ms":second_ms,"results_identical":true,
        "scope":"Single local warm/cold sample, native process cache; not a cross-version cold speedup claim"})
    );
}
