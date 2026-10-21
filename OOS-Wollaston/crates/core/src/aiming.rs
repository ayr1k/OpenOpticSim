//! Connected-chief-branch shooting. Never export Newton trial paths as physical rays.
use crate::{AimInfo, PreparedTrace, Project, RayPath, RayStatus, TraceStage};
use nalgebra::{Matrix2, Vector2};

#[derive(Clone)]
struct Solution {
    xy: Vector2<f64>,
    jac: Matrix2<f64>,
    residual: f64,
}
#[derive(Clone)]
struct Failure {
    residual: Option<f64>,
    reason: &'static str,
    surface: Option<usize>,
    status: Option<RayStatus>,
}
impl Failure {
    fn unresolved() -> Self {
        Self {
            residual: None,
            reason: "target_unresolved",
            surface: None,
            status: None,
        }
    }
}
pub struct PupilAimer<'a> {
    p: &'a Project,
    field: f64,
    nm: f64,
    seed_a: f64,
    seed_b: f64,
    chief: Option<Result<Solution, Failure>>,
    iterations: usize,
    prepared: PreparedTrace<'a>,
}
impl<'a> PupilAimer<'a> {
    pub fn new(p: &'a Project, field: f64, nm: f64) -> Self {
        let mut m = Matrix2::identity();
        let mut n = 1.;
        for (i, s) in p.surfaces.iter().enumerate() {
            if i == p.stop {
                break;
            }
            let n2 = p
                .materials
                .get(s.material)
                .and_then(|m| m.index(nm).ok())
                .unwrap_or(n);
            m = Matrix2::new(1., 0., -(n2 - n) * s.curvature, 1.) * m;
            m = Matrix2::new(1., s.thickness / n2, 0., 1.) * m;
            n = n2;
        }
        let prepared = PreparedTrace::new(p, nm);
        let (origin, _) = prepared.launch(field, [0., 0.]);
        let a = m[(0, 0)];
        let b = m[(0, 1)] - a * origin.z;
        let (seed_a, seed_b) = if let Some(dist) = p.object_distance {
            (
                a + b / (dist + origin.z),
                b * dist * field.tan() / (dist + origin.z),
            )
        } else {
            (a, b * field.tan())
        };
        Self {
            p,
            field,
            nm,
            seed_a,
            seed_b,
            chief: None,
            iterations: 0,
            prepared,
        }
    }
    fn evaluate(&self, xy: Vector2<f64>) -> Result<Vector2<f64>, Failure> {
        let (origin, direction) = self.prepared.launch(self.field, xy.into());
        let r = self
            .prepared
            .trace(origin, direction, false, Some(self.p.stop));
        if r.status != RayStatus::Complete {
            return Err(Failure {
                residual: None,
                reason: "trial_trace_failed",
                surface: r.surface,
                status: Some(r.status),
            });
        }
        let h = r.points.last().ok_or_else(Failure::unresolved)?;
        Ok(Vector2::new(h[0], h[1]))
    }
    fn jacobian(&self, xy: Vector2<f64>, at: Vector2<f64>) -> Result<Matrix2<f64>, Failure> {
        let h = 1e-5_f64.max(xy.norm() * 1e-7);
        let mut j = Matrix2::zeros();
        for axis in 0..2 {
            let mut plus = xy;
            let mut minus = xy;
            plus[axis] += h;
            minus[axis] -= h;
            let a = self.evaluate(plus);
            let b = self.evaluate(minus);
            let col = match (a, b) {
                (Ok(a), Ok(b)) => (a - b) / (2. * h),
                (Ok(a), Err(_)) => (a - at) / h,
                (Err(_), Ok(b)) => (at - b) / h,
                (Err(e), Err(_)) => return Err(e),
            };
            j.set_column(axis, &col);
        }
        Ok(j)
    }
    fn solve(&mut self, mut xy: Vector2<f64>, target: Vector2<f64>) -> Result<Solution, Failure> {
        let radius = self.p.surfaces[0].semi_aperture.max(1.);
        let trust = (radius * 0.35).max(1.);
        let guard = (radius * 4. + xy.norm()).clamp(10., 100000.);
        let mut failure = Failure::unresolved();
        for _ in 0..32 {
            self.iterations += 1;
            let at = self.evaluate(xy)?;
            let e = at - target;
            let residual = e.norm();
            failure.residual = Some(residual);
            let j = self.jacobian(xy, at)?;
            if residual < 1e-8 {
                return Ok(Solution {
                    xy,
                    jac: j,
                    residual,
                });
            }
            let Some(mut step) = j.lu().solve(&e) else {
                failure.reason = "singular_mapping";
                return Err(failure);
            };
            if !step.iter().all(|x| x.is_finite()) {
                return Err(failure);
            }
            if step.norm() > trust {
                step *= trust / step.norm();
            }
            let mut accepted = false;
            let mut alpha = 1.;
            for _ in 0..13 {
                let trial = xy - step * alpha;
                if trial.norm() <= guard {
                    match self.evaluate(trial) {
                        Ok(v)
                            if (v - target).norm() < residual * (1. - 1e-4 * alpha)
                                || (v - target).norm() < 1e-8 =>
                        {
                            xy = trial;
                            accepted = true;
                            break;
                        }
                        Err(err) => {
                            failure.surface = err.surface;
                            failure.status = err.status;
                        }
                        _ => {}
                    }
                }
                alpha *= 0.5;
            }
            if !accepted {
                return Err(failure);
            }
        }
        Err(failure)
    }
    fn connected(chief: &Solution, solution: &Solution) -> bool {
        let Some(inv) = chief.jac.try_inverse() else {
            return false;
        };
        let relative = solution.jac * inv;
        // Stable orientation on both pupil axes relative to the chief branch.
        relative.determinant() > 1e-8 && relative.trace() > 0.
    }
    fn failure_path(&self, target: [f64; 2], failure: Failure, method: &str) -> RayPath {
        RayPath {
            points: vec![],
            direction: [0., 0., 1.],
            optical_path: 0.,
            wavelength_nm: self.nm,
            status: RayStatus::AimingFailed,
            surface: Some(self.p.stop),
            stage: TraceStage::Aiming,
            aiming: Some(AimInfo {
                target_mm: target,
                residual_mm: failure.residual,
                iterations: self.iterations,
                method: method.into(),
                failure: Some(failure.reason.into()),
                trial_surface: failure.surface,
                trial_status: failure.status,
            }),
        }
    }
    pub fn aim(&mut self, target: [f64; 2], clip: bool) -> RayPath {
        self.iterations = 0;
        if self.chief.is_none() {
            let y = if self.seed_a.abs() > 1e-10 {
                -self.seed_b / self.seed_a
            } else {
                0.
            };
            let first = self.solve(Vector2::new(0., y), Vector2::zeros());
            let mut result = if first.is_err() && y.abs() > 1e-6 {
                self.solve(Vector2::zeros(), Vector2::zeros())
            } else {
                first
            };
            if result.is_err() && self.field.abs() > 0.15 {
                // Continue the axial chief in object angle before declaring an
                // unresolved wide-field pupil. Physical clipping remains a
                // separate final trace; failed trials still have no path.
                let wanted = self.field;
                let steps = (wanted.abs() / 0.025).ceil() as usize;
                self.field = 0.;
                let mut seed = Vector2::zeros();
                let mut previous = Vector2::zeros();
                for i in 0..=steps {
                    self.field = wanted * i as f64 / steps as f64;
                    match self.solve(seed, Vector2::zeros()) {
                        Ok(s) => {
                            let prediction = s.xy + (s.xy - previous);
                            previous = s.xy;
                            seed = prediction;
                            result = Ok(s);
                        }
                        Err(e) => {
                            result = Err(e);
                            break;
                        }
                    }
                }
                self.field = wanted;
            }
            self.chief = Some(result);
        }
        let chief = match self.chief.as_ref().unwrap().clone() {
            Ok(v) => v,
            Err(mut e) => {
                e.reason = "chief_unresolved";
                return self.failure_path(target, e, "chief_search");
            }
        };
        let Some(inv) = chief.jac.try_inverse() else {
            let mut e = Failure::unresolved();
            e.reason = "singular_mapping";
            return self.failure_path(target, e, "chief_search");
        };
        let goal = Vector2::from(target);
        let mut method = "damped_newton";
        let direct = if goal.norm() < 1e-15 {
            Ok(chief.clone())
        } else {
            self.solve(chief.xy + inv * goal, goal)
        };
        let mut result = match direct {
            Ok(s) if Self::connected(&chief, &s) => Ok(s),
            Ok(_) => {
                let mut e = Failure::unresolved();
                e.reason = "folded_mapping";
                Err(e)
            }
            Err(e) => Err(e),
        };
        if result.is_err() && goal.norm() > 1e-15 {
            method = "chief_continuation";
            let mut last = chief.clone();
            for i in 1..=8 {
                let previous_goal = goal * ((i - 1) as f64 / 8.);
                let next_goal = goal * (i as f64 / 8.);
                let next_seed =
                    last.xy + last.jac.try_inverse().unwrap_or(inv) * (next_goal - previous_goal);
                match self.solve(next_seed, next_goal) {
                    Ok(s) if Self::connected(&chief, &s) => {
                        last = s;
                        result = Ok(last.clone());
                    }
                    Ok(_) => {
                        let mut e = Failure::unresolved();
                        e.reason = "folded_mapping";
                        result = Err(e);
                        break;
                    }
                    Err(e) => {
                        result = Err(e);
                        break;
                    }
                }
            }
        }
        match result {
            Err(e) => self.failure_path(target, e, method),
            Ok(s) => {
                let (origin, direction) = self.prepared.launch(self.field, s.xy.into());
                let mut ray = self.prepared.trace(origin, direction, clip, None);
                ray.aiming = Some(AimInfo {
                    target_mm: target,
                    residual_mm: Some(s.residual),
                    iterations: self.iterations,
                    method: method.into(),
                    failure: None,
                    trial_surface: None,
                    trial_status: None,
                });
                ray
            }
        }
    }
}
