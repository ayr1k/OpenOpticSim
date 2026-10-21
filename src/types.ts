export interface Material {
  name: string;
  n: number;
  sellmeier: number[][] | null;
  range_nm: [number, number];
  source: string;
}
export interface FieldPlan {
  efl_mm: number | null;
  reference_wavelength_nm: number;
  geometry_limit_rad: number;
  fields: {
    label: string;
    target_height_mm: number | null;
    initial_angle_rad: number | null;
    angle_rad: number | null;
    actual_height_mm: number | null;
    residual_mm: number | null;
    chief_status: string;
    diagnostic: string | null;
  }[];
}
export interface Coverage {
  sensor_mm: number[];
  half_diagonal_mm: number;
  max_tested_height_mm: number | null;
  reference_wavelength_nm: number;
  sampling: number;
  image_z_mm: number;
  focus_condition: string;
  fields: {
    angle_rad: number;
    chief_height_mm: number | null;
    sampled: number;
    blocked: number;
    unresolved: number;
    other_failed: number;
    geometric_fraction: number;
    fast_mtf: string;
    wide_mtf: string;
  }[];
  checkpoints: {
    label: string;
    height_mm: number;
    ideal_angle_rad: number | null;
    tested_field_index: number | null;
    status: string;
  }[];
  warnings: string[];
}
export interface QualityGuards {
  require_sensor_coverage?: boolean;
  max_rms_um: (number | null)[];
  max_worst_rms_um?: number | null;
  max_degradation_percent?: number | null;
  max_distortion_percent?: number | null;
  max_lateral_color_um?: number | null;
  min_geometric_fraction?: number | null;
}
export interface Surface {
  coating?: import("./coatings").CoatingLayer[];
  name: string;
  curvature: number;
  conic: number;
  asphere: [number, number, number, number];
  thickness: number;
  semi_aperture: number;
  material: number;
  mechanical?: { semi_diameter: number; shape: "flat" | "curved" };
}
export interface Geometry {
  spans: {
    front: number;
    back: number;
    glass: boolean;
    center_mm: number;
    edge_mm: number | null;
    sampled_min_mm: number | null;
    radius_mm: number;
    certified: boolean;
  }[];
  diagnostics: { surface: number | null; message: string }[];
}
export interface Project {
  schema_version: number;
  name: string;
  description: string;
  materials: Material[];
  surfaces: Surface[];
  stop: number;
  object_distance: number | null;
  sensor: [number, number];
  fields: number[];
  field_spec?:
    { mode: "sensor" } | { mode: "image_heights"; heights_mm: number[] };
  wavelengths: { nm: number; weight: number }[];
  workspace?: Workspace;
}
export interface Ray {
  points: number[][];
  status: string;
  surface: number | null;
  wavelength_nm: number;
  stage?: "aiming" | "trace";
  aiming?: {
    target_mm: number[];
    residual_mm: number | null;
    iterations: number;
    method: string;
    failure: string | null;
    trial_surface: number | null;
    trial_status: string | null;
  } | null;
}
export interface RayFailure {
  stage: "aiming" | "trace";
  status: string;
  surface: number | null;
  reason: string | null;
  count: number;
}
export interface Spot {
  field: number;
  wavelength_nm: number;
  weight: number;
  points: number[][];
  centroid: number[] | null;
  rms_um: number | null;
  blocked_fraction: number;
  failed: number;
  sampled: number;
  distortion_percent: number | null;
  failures?: RayFailure[];
}
export interface WaveOptions {
  method?: "fast" | "sphere_kirchhoff";
  field_index: number;
  wavelength_nm: number | null;
  max_pupil_samples: number;
}
export interface WaveBand {
  wavelength_nm: number;
  weight: number;
  rms_waves: number;
  chief_image_mm: number[];
  pupil_samples: number;
  blocked_samples: number;
  max_phase_step_waves: number;
}
export interface WaveQuality {
  image_nyquist_ratio?: number | null;
  converged: boolean;
  pupil_sampling: number;
  previous_sampling: number;
  mtf_max_change: number;
  psf_l1_change: number;
  edge_energy: number;
  local_expansion_error_waves: number;
}
export interface Wave {
  coverage?: Coverage | null;
  method?: "fast" | "sphere_kirchhoff";
  size: number;
  psf: number[];
  mtf_sagittal: number[];
  mtf_tangential: number[];
  pixel_um: number;
  frequency_step_lp_mm: number;
  field: number;
  reference_image_mm: number[];
  bands: WaveBand[];
  quality: WaveQuality;
  warning: string;
}
export interface Analysis {
  coverage?: Coverage | null;
  paraxial: {
    efl: number | null;
    bfl: number | null;
    entrance_pupil_z: number | null;
    entrance_pupil_radius: number | null;
    exit_pupil_z: number | null;
    exit_pupil_radius: number | null;
    f_number: number | null;
  };
  spots: Spot[];
  fields: {
    field: number;
    centroid: number[] | null;
    polychromatic_rms_um: number | null;
    lateral_color_um: number | null;
    sagittal_focus_offset_mm: number | null;
    tangential_focus_offset_mm: number | null;
  }[];
  chromatic_focus: {
    wavelength_nm: number;
    paraxial_focus_offset_mm: number | null;
  }[];
  rays: Ray[];
  diagnostics: { surface: number | null; message: string }[];
  sampling: number;
  units: string;
  experimental_wave: Wave | null;
}
export interface Candidate {
  project: Project;
  before_rms_um: number | null;
  after_rms_um: number;
  iterations: number;
  note: string;
  accepted: boolean;
  checks: ConstraintCheck[];
  sampling: number;
  report?: OptimizationReport | null;
}
export interface Variable {
  kind: "curvature" | "thickness" | "asphere" | "aperture";
  surface: number;
  coefficient?: number;
}
export type AnalysisPreset = "center" | "balanced" | "edge";
export interface VariableSetting {
  variable: Variable;
  min: number | null;
  max: number | null;
  locked: boolean;
}
export interface OptimizationOptions {
  quality?: QualityGuards;
  staged?: boolean;
  refocus_each_stage?: boolean;
  variables: VariableSetting[];
  preset: AnalysisPreset;
  max_iterations: number;
}
export interface OptimizationProgress {
  iteration: number;
  max_iterations: number;
  merit: number;
  weighted_rms_um: number;
  efl_mm: number | null;
  total_length_mm: number;
  step_accepted: boolean;
}
export interface OptimizationReport {
  focus_condition?: string;
  before_coverage?: Coverage | null;
  after_coverage?: Coverage | null;
  stages?: {
    field_scale: number;
    original_fields_rms_um: (number | null)[];
    candidate_fields_rms_um: (number | null)[];
    accepted: boolean;
  }[];
  preset: AnalysisPreset;
  field_weights: number[];
  before_fields_rms_um: (number | null)[];
  after_fields_rms_um: (number | null)[];
  before_weighted_rms_um: number | null;
  after_weighted_rms_um: number | null;
  variables: VariableSetting[];
  history: OptimizationProgress[];
  stop_reason: string;
}

export interface FocusOptions {
  min_mm: number;
  max_mm: number;
  steps: number;
  sampling: number;
  criterion: "center" | "balanced";
}
export interface FocusScan {
  options: FocusOptions;
  fields_rad: number[];
  best_index: number | null;
  boundary_best: boolean;
  points: {
    coverage?: Coverage | null;
    fields_rad?: number[];
    image_distance_mm: number;
    fields_rms_um: (number | null)[];
    score_um: number | null;
    failed: number;
    sampled: number;
    diagnostic: string | null;
  }[];
}
export interface Constraints {
  target_efl_mm: number;
  efl_tolerance_mm: number;
  max_length_mm: number;
  min_center_mm: number;
  min_edge_mm: number;
  min_air_mm: number;
  design?: DesignTargets;
}
export interface DesignTargets {
  f_number: number;
  f_tolerance: number;
  min_bfl_mm: number;
  max_front_diameter_mm: number;
}
export interface ApertureOptions {
  wave_method?: WaveOptions["method"];
  f_numbers: number[];
  refocus: boolean;
  sampling: number;
  mtf: boolean;
}
export interface AperturePoint {
  coverage?: Coverage | null;
  fields_rad?: number[];
  requested_f_number: number;
  actual_f_number: number | null;
  stop_radius_mm: number | null;
  image_distance_mm: number;
  fields_rms_um: (number | null)[];
  score_um: number | null;
  failed: number;
  sampled: number;
  aiming_failed: number;
  blocked: number;
  mtf: {
    method?: WaveOptions["method"];
    field: number;
    values: (number | null)[] | null;
    quality: WaveQuality | null;
    error: string | null;
  }[];
  diagnostic: string | null;
}
export interface ApertureScan {
  source: Prescription;
  options: ApertureOptions;
  points: AperturePoint[];
  best_index: number | null;
}
export interface OpticalGroup {
  id: string;
  name: string;
  start: number;
  end: number;
}
export interface StopMap {
  field: number;
  wavelength_nm: number;
  radius_mm: number;
  sampling: number;
  samples: { target_mm: number[]; ray: Ray }[];
}
export interface ConstraintCheck {
  name: string;
  surface: number | null;
  actual: number | null;
  limit: number;
  relation: "min" | "max";
  passed: boolean;
  reason?: string;
}
export type Prescription = Omit<Project, "workspace">;
export interface DesignVersion {
  motion?: MotionWorkspace;
  id: string;
  name: string;
  created_at: string;
  note: string;
  prescription: Prescription;
  analysis_preset?: AnalysisPreset;
  groups?: OpticalGroup[];
}
export interface Workspace {
  design_task?: import("./designTask").DesignBrief;
  motion?: MotionWorkspace;
  schema_version: 1;
  versions: DesignVersion[];
  focus?: FocusOptions;
  constraints?: Constraints;
  optimization?: OptimizationOptions;
  aperture?: ApertureOptions;
  aperture_scan?: ApertureScan;
  groups?: OpticalGroup[];
}

export interface MotionGroup extends OpticalGroup {
  min_mm: number;
  max_mm: number;
  locked: boolean;
}
export interface MotionConfiguration {
  id: string;
  name: string;
  positions_mm: number[];
  object_distance: number | null;
  target_efl_mm: number | null;
  target_f_number: number | null;
}
export interface MotionWorkspace {
  study?: StudyWorkspace;
  application_revision?: number;
  schema_version: 1;
  reference: Prescription;
  groups: MotionGroup[];
  configurations: MotionConfiguration[];
  active_id: string | null;
  min_air_mm: number;
  sampling: number;
  criterion: "center" | "balanced";
  max_rms_um: number;
}
export interface MotionOptions {
  groups: MotionGroup[];
  positions_mm: number[];
  object_distance: number | null;
  min_air_mm: number;
  target_f_number: number | null;
  solve_group: number | null;
  sampling: number;
  criterion: "center" | "balanced";
  max_rms_um: number;
}
export interface MotionResult {
  project: Prescription;
  positions_mm: number[];
  origin_shift_mm: number;
  sensor_reference_mm: number;
  analysis: Analysis;
  score_um: number | null;
  accepted: boolean;
  boundary_best: boolean;
  diagnostic: string;
  points: {
    positions_mm: number[];
    score_um: number | null;
    diagnostic: string | null;
  }[];
}

export type StudyVariable =
  | {
      kind: "curvature" | "thickness";
      surface: number;
      min: number;
      max: number;
    }
  | {
      kind: "position";
      configuration: number;
      group: number;
      min: number;
      max: number;
    };
export interface StudySettings {
  mode: "scan" | "trajectory" | "focus" | "optimize";
  steps: number;
  max_points: number;
  adaptive: boolean;
  refine_rms_um: number;
  driver_group: number;
  compensator_group: number;
  near_mm: number;
  far_mm: number;
  include_infinity: boolean;
  verification_sampling: number;
  max_degradation_percent: number;
  efl_tolerance_percent: number | null;
  min_glass_mm: number;
  require_sensor_coverage: boolean;
  variables: StudyVariable[];
  iterations: number;
}
export interface StudyOptions extends StudySettings {
  motion: MotionOptions;
  configurations: MotionConfiguration[];
}
export interface StudyPoint {
  coordinate: number;
  configuration: MotionConfiguration;
  accepted: boolean;
  diagnostic: string;
  rms_um: number | null;
  fields_rms_um: (number | null)[];
  efl_mm: number | null;
  f_number: number | null;
  distortion_percent: number | null;
  lateral_color_um: number | null;
  geometric_fraction: number | null;
  min_air_mm: number | null;
  min_glass_mm: number | null;
  field_angles_rad: number[];
  coverage: Analysis["coverage"];
  failed_rays: number;
}
export interface StudyResult {
  mode: StudySettings["mode"];
  reference: Prescription;
  configurations: MotionConfiguration[];
  before: StudyPoint[];
  points: StudyPoint[];
  accepted: boolean;
  verification_sampling: number;
  evaluations: number;
  sensor_reference_mm: number;
  note: string;
}
export interface StudyWorkspace {
  settings: StudySettings;
  saved?: { source_key: string; settings: StudySettings; result: StudyResult };
}
