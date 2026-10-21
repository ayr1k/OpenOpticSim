use openoptics::*;
use std::sync::atomic::AtomicBool;
#[test]
fn coating_fresnel_quarter_wave_brewster_and_energy() {
    use coating::*;
    let bare = response(1., 1.5, 0., 550., &[]).unwrap();
    assert!((bare.rs - 0.04).abs() < 1e-14);
    assert!((bare.ts - 0.96).abs() < 1e-14);
    let n = 1.5f64.sqrt();
    let layers = vec![Layer {
        name: "analytic ideal".into(),
        index: n,
        thickness_nm: 550. / (4. * n),
    }];
    assert!(response(1., 1.5, 0., 550., &layers).unwrap().rs < 1e-28);
    let brewster = response(1., 1.5, 1.5f64.atan(), 550., &[]).unwrap();
    assert!(brewster.rp < 1e-28);
    for angle in [0., 0.2, 0.7, 1.2] {
        for nm in [400., 550., 700.] {
            let r = response(1., 1.5, angle, nm, &layers).unwrap();
            assert!((r.rs + r.ts - 1.).abs() < 1e-12);
            assert!((r.rp + r.tp - 1.).abs() < 1e-12);
        }
    }
    assert!(response(1.5, 1., 1., 550., &[]).is_err());
}
#[test]
fn coatings_roundtrip_do_not_change_geometric_paths() {
    let mut p = examples().remove(0);
    let cancel = AtomicBool::new(false);
    let before = serde_json::to_value(analyze(&p, 7, &cancel).unwrap()).unwrap();
    p.surfaces[1].coating = vec![coating::Layer {
        name: "L".into(),
        index: 1.38,
        thickness_nm: 100.,
    }];
    let encoded = serde_json::to_string(&p).unwrap();
    let reopened: Project = serde_json::from_str(&encoded).unwrap();
    assert_eq!(reopened.surfaces[1].coating[0].thickness_nm, 100.);
    assert_eq!(
        before,
        serde_json::to_value(analyze(&reopened, 7, &cancel).unwrap()).unwrap()
    );
    let r = coating::analyze_coatings(
        &reopened,
        &coating::CoatingOptions {
            stacks: vec![coating::Stack {
                surface: 1,
                layers: reopened.surfaces[1].coating.clone(),
            }],
        },
        &cancel,
    )
    .unwrap();
    assert!(r["rays"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["transmission"]
            .as_f64()
            .zip(v["uncoated"].as_f64())
            .is_some_and(|(a, b)| a > b)));
}
#[test]
fn classic_topologies_and_cancelled_search() {
    for (kind, count) in [
        ("tessar", 8),
        ("planar", 11),
        ("sonnar", 10),
        ("double_gauss", 13),
    ] {
        let p = synthesis::seed(kind).unwrap();
        assert_eq!(p.surfaces.len(), count);
        assert!(p.validate().is_empty(), "{kind}: {:?}", p.validate());
        let brief = synthesis::DesignBrief {
            exploration: synthesis::Exploration::Fixed,
            element_glass: false,
            max_attempts: 36,
            min_center_mm: 0.5,
            min_edge_mm: 0.1,
            require_sensor_coverage: false,
            glass_search: false,
            structure: kind.into(),
            focal_length_mm: 50.,
            f_number: 8.,
            sensor_mm: [36., 24.],
            half_field_deg: 5.,
            max_length_mm: 150.,
            max_rms_um: 25.,
            iterations: 1,
            starts: 1,
        };
        assert!(synthesis::run(&brief, &AtomicBool::new(true), &mut |_| {}).is_err());
    }
}
#[test]
fn cached_wave_is_exact_and_cancellation_is_not_bypassed() {
    let p = examples().remove(1);
    let cancel = AtomicBool::new(false);
    let p = autofocus(&p, &cancel).unwrap().project;
    let options = wave::WaveOptions::default();
    let a = wave::diffraction_uncached(&p, &options, &cancel).unwrap();
    let b = wave::diffraction(&p, &options, &cancel).unwrap();
    let c = wave::diffraction(&p, &options, &cancel).unwrap();
    assert_eq!(
        serde_json::to_value(a).unwrap(),
        serde_json::to_value(&b).unwrap()
    );
    assert_eq!(
        serde_json::to_value(b).unwrap(),
        serde_json::to_value(c).unwrap()
    );
    assert!(wave::diffraction(&p, &options, &AtomicBool::new(true)).is_err());
}
#[test]
fn thirty_two_variables_are_supported_and_thirty_three_are_rejected() {
    let mut p = synthesis::seed("double_gauss").unwrap();
    p.fields = vec![0.];
    p.surfaces[p.stop].semi_aperture = 1.;
    let vars: Vec<_> = (0..32)
        .map(|i| VariableSetting {
            variable: Variable::Asphere {
                surface: i / 4,
                coefficient: i % 4,
            },
            min: Some(-1e-12),
            max: Some(1e-12),
            locked: false,
        })
        .collect();
    let mut o = OptimizationOptions {
        quality: None,
        staged: false,
        refocus_each_stage: false,
        variables: vars,
        preset: AnalysisPreset::Balanced,
        max_iterations: 1,
    };
    let c = Constraints {
        target_efl_mm: 50.,
        efl_tolerance_mm: 1.,
        max_length_mm: 200.,
        min_center_mm: 0.,
        min_edge_mm: 0.,
        min_air_mm: 0.,
        design: None,
    };
    let result = optimize_configured(&p, &o, &c, &AtomicBool::new(false), &mut |_| {});
    assert!(result.is_ok(), "{:?}", result.err());
    o.variables.push(VariableSetting {
        variable: Variable::Curvature { surface: 0 },
        min: None,
        max: None,
        locked: false,
    });
    assert!(optimize_configured(&p, &o, &c, &AtomicBool::new(false), &mut |_| {}).is_err());
}
