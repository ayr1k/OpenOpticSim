pub mod aiming;
pub mod analysis;
pub mod aperture;
pub mod coating;
pub mod constraints;
pub mod fields;
pub mod focus;
pub mod geometry;
pub mod model;
pub mod motion;
pub mod optimize;
pub mod preview;
pub mod quality;
pub mod restoration;
pub mod study;
pub mod synthesis;
pub mod trace;
pub mod wave;
pub use analysis::*;
pub use constraints::*;
pub use focus::*;
pub use model::*;
pub use optimize::*;
pub use trace::*;

#[derive(serde::Deserialize)]
pub struct Request {
    pub operation: String,
    pub project: Project,
    #[serde(default = "default_sampling")]
    pub sampling: usize,
    #[serde(default)]
    pub variables: Vec<Variable>,
    pub target_efl: Option<f64>,
    #[serde(default)]
    pub diffraction: bool,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub wave_options: wave::WaveOptions,
    pub focus_options: Option<FocusOptions>,
    pub constraints: Option<Constraints>,
    pub optimization: Option<OptimizationOptions>,
    pub aperture_options: Option<aperture::ApertureOptions>,
    pub group_move: Option<aperture::GroupMove>,
    pub motion_options: Option<motion::MotionOptions>,
    pub study_options: Option<study::StudyOptions>,
    pub design_brief: Option<synthesis::DesignBrief>,
    pub search_checkpoint: Option<synthesis::Checkpoint>,
    pub coating_options: Option<coating::CoatingOptions>,
}
fn default_sampling() -> usize {
    17
}
pub fn execute(
    request: Request,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<serde_json::Value, String> {
    execute_with_progress(request, cancel, &mut |_| {})
}
pub fn execute_with_progress(
    request: Request,
    cancel: &std::sync::atomic::AtomicBool,
    progress: &mut dyn FnMut(&OptimizationProgress),
) -> Result<serde_json::Value, String> {
    let p = &request.project;
    if request.operation == "synthesize_step" {
        return Ok(serde_json::json!(synthesis::step(
            request
                .design_brief
                .as_ref()
                .ok_or("Design brief required")?,
            request.search_checkpoint.as_ref(),
            cancel,
            progress
        )?));
    }
    if request.operation == "synthesize" {
        return Ok(serde_json::json!(synthesis::run(
            request
                .design_brief
                .as_ref()
                .ok_or("Design brief required")?,
            cancel,
            progress
        )?));
    }
    if request.operation == "validate" {
        return Ok(serde_json::json!(p.validate()));
    }
    if request.operation == "geometry" {
        let errors = p.validate_structure();
        if !errors.is_empty() {
            return Ok(serde_json::json!(geometry::Geometry {
                spans: vec![],
                diagnostics: errors
            }));
        }
        return Ok(serde_json::json!(geometry::inspect(p)));
    }
    let errors = if request.operation == "optimize" && request.optimization.is_some() {
        p.validate_structure()
    } else {
        p.validate()
    };
    if !errors.is_empty() {
        return Err(serde_json::to_string(&errors).unwrap());
    }
    if p.surfaces.is_empty() {
        return Err("Add a lens before optical analysis".into());
    }
    match request.operation.as_str() {
        "coatings" => coating::analyze_coatings(
            p,
            &request
                .coating_options
                .unwrap_or_else(|| coating::CoatingOptions {
                    stacks: p
                        .surfaces
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| !s.coating.is_empty())
                        .map(|(i, s)| coating::Stack {
                            surface: i,
                            layers: s.coating.clone(),
                        })
                        .collect(),
                }),
            cancel,
        ),
        "motion_study" => Ok(serde_json::json!(study::run(
            p,
            request
                .study_options
                .as_ref()
                .ok_or("Study options required")?,
            cancel,
            progress
        )?)),
        "motion" => Ok(serde_json::json!(motion::run(
            p,
            request
                .motion_options
                .as_ref()
                .ok_or("Motion options required")?,
            cancel
        )?)),
        "field_plan" => Ok(serde_json::json!(fields::plan(p, cancel)?)),
        "aperture_scan" => Ok(serde_json::json!(aperture::aperture_scan(
            p,
            request
                .aperture_options
                .as_ref()
                .ok_or("Aperture options required")?,
            cancel
        )?)),
        "stop_map" => Ok(serde_json::json!(aperture::stop_map(
            &fields::resolve(p, cancel)?,
            request.wave_options.field_index,
            request.wave_options.wavelength_nm.unwrap_or(587.5618),
            request.sampling,
            cancel
        )?)),
        "move_group" => Ok(serde_json::json!(aperture::move_group(
            p,
            request.group_move.as_ref().ok_or("Group move required")?
        )?)),
        "checks" => {
            let c = request.constraints.as_ref().ok_or("Constraints required")?;
            c.validate()?;
            Ok(serde_json::json!(constraint_checks(p, c)))
        }
        "analyze" | "trace" => {
            let mut a = analyze(p, request.sampling, cancel)?;
            if request.diffraction {
                a.experimental_wave = Some(wave::diffraction(p, &request.wave_options, cancel)?);
            }
            Ok(serde_json::json!(a))
        }
        "autofocus" => Ok(serde_json::json!(autofocus(p, cancel)?)),
        "focus_scan" => Ok(serde_json::json!(focus_scan(
            p,
            request
                .focus_options
                .as_ref()
                .ok_or("Focus options required")?,
            cancel
        )?)),
        "wave" => Ok(serde_json::json!(wave::diffraction(
            p,
            &request.wave_options,
            cancel
        )?)),
        "pupils" => Ok(serde_json::json!(wave::pupils(
            p,
            &request.wave_options,
            request.sampling,
            cancel
        )?)),
        "pupil" => {
            let q = fields::resolve(p, cancel)?;
            Ok(serde_json::json!(wave::pupil_grid(
                &q,
                *q.fields
                    .get(request.wave_options.field_index)
                    .ok_or("Wave field index out of range")?,
                request.wave_options.wavelength_nm.unwrap_or(587.5618),
                request.sampling,
                cancel
            )?))
        }
        "optimize" if request.optimization.is_some() => Ok(serde_json::json!(optimize_configured(
            p,
            request.optimization.as_ref().unwrap(),
            request
                .constraints
                .as_ref()
                .ok_or("Optimization constraints required")?,
            cancel,
            progress
        )?)),
        "optimize" if request.constraints.is_some() => Ok(serde_json::json!(optimize_constrained(
            p,
            &request.variables,
            request.constraints.as_ref().unwrap(),
            cancel
        )?)),
        "optimize" => Ok(serde_json::json!(optimize(
            p,
            &request.variables,
            request.target_efl.ok_or("Target EFL required")?,
            cancel
        )?)),
        "preview" => Ok(serde_json::json!(preview::preview_with_options(
            p,
            request.pattern.as_deref().unwrap_or("grid"),
            &request.wave_options,
            cancel
        )?)),
        _ => Err("Unknown operation".into()),
    }
}
