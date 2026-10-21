//! Per-field acceptance guards; failed rays can never improve a quality score.
use crate::*;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QualityGuards {
    pub require_sensor_coverage: bool,
    pub max_rms_um: Vec<Option<f64>>,
    pub max_worst_rms_um: Option<f64>,
    pub max_degradation_percent: Option<f64>,
    pub max_distortion_percent: Option<f64>,
    pub max_lateral_color_um: Option<f64>,
    pub min_geometric_fraction: Option<f64>,
}
impl QualityGuards {
    pub fn validate(&self, n: usize) -> Result<(), String> {
        if (!self.max_rms_um.is_empty() && self.max_rms_um.len() != n)
            || self
                .max_rms_um
                .iter()
                .chain([
                    &self.max_worst_rms_um,
                    &self.max_degradation_percent,
                    &self.max_distortion_percent,
                    &self.max_lateral_color_um,
                ])
                .flatten()
                .any(|v| !v.is_finite() || *v < 0.)
            || self
                .min_geometric_fraction
                .is_some_and(|v| !v.is_finite() || !(0. ..=1.).contains(&v))
        {
            return Err(
                "画质保护设置无效；逐视场上限须对应全部视场，阈值非负，几何采样比例为 0–1".into(),
            );
        }
        Ok(())
    }
}
pub fn checks(a: &Analysis, before: &Analysis, g: &QualityGuards) -> Vec<ConstraintCheck> {
    let mut out = Vec::new();
    let mut push = |name: String, actual: Option<f64>, limit: f64, relation: &str, reason: &str| {
        out.push(ConstraintCheck {
            name,
            surface: None,
            actual,
            limit,
            relation: relation.into(),
            passed: actual.is_some_and(|v| {
                if relation == "min" {
                    v + 1e-9 >= limit
                } else {
                    v <= limit + 1e-9
                }
            }),
            reason: Some(reason.into()),
        })
    };
    for (i, f) in a.fields.iter().enumerate() {
        let label = format!("视场 {} ({:.3}°)", i + 1, f.field.to_degrees());
        let selected: Vec<_> = a.spots.iter().filter(|s| s.field == f.field).collect();
        let numerical_failure = selected
            .iter()
            .flat_map(|s| &s.failures)
            .any(|e| e.status != RayStatus::Blocked);
        let rms = if numerical_failure {
            None
        } else {
            f.polychromatic_rms_um
        };
        if let Some(limit) = g.max_rms_um.get(i).copied().flatten() {
            push(
                format!("{label} RMS µm"),
                rms,
                limit,
                "max",
                "多色 RMS；数值失败不会按零误差计分。",
            );
        }
        if let Some(percent) = g.max_degradation_percent {
            let baseline = before.fields.get(i).and_then(|f| f.polychromatic_rms_um);
            let limit = baseline.map(|v| v * (1. + percent / 100.)).unwrap_or(0.);
            push(
                format!("{label} 原设计保护 RMS µm"),
                if baseline.is_some() { rms } else { None },
                limit,
                "max",
                "与同一视场定义、波长、瞳采样和对焦条件的原设计比较；平均值改善不能覆盖此项。",
            );
        }
        if let Some(limit) = g.max_distortion_percent {
            let distortion = if f.field.abs() < 1e-10 {
                Some(0.)
            } else {
                selected
                    .iter()
                    .map(|s| s.distortion_percent.map(f64::abs))
                    .collect::<Option<Vec<_>>>()
                    .and_then(|v| v.into_iter().reduce(f64::max))
            };
            push(
                format!("{label} 最大绝对畸变 %"),
                distortion,
                limit,
                "max",
                "无穷远以 EFL·tan(物方角度) 为理想像高；有限物距未定义时禁止通过。",
            );
        }
        if let Some(limit) = g.max_lateral_color_um {
            push(
                format!("{label} 横向色差 µm"),
                if selected.len() == 1 {
                    Some(0.)
                } else {
                    f.lateral_color_um
                },
                limit,
                "max",
                "全部正权重波段的质心跨度；此项不是主光线色差。",
            );
        }
        if let Some(limit) = g.min_geometric_fraction {
            let sampled = selected.iter().map(|s| s.sampled).sum::<usize>();
            let failed = selected.iter().map(|s| s.failed).sum::<usize>();
            let fraction = if numerical_failure || sampled == 0 {
                None
            } else {
                Some(1. - failed as f64 / sampled as f64)
            };
            push(format!("{label} 几何采样通过比例"),fraction,limit,"min","仅物理遮挡可计入损失；瞄准、求交、材料及全反射失败禁止判为通过。不等同于精确通量或相对照度。");
        }
    }
    if let Some(limit) = g.max_worst_rms_um {
        let actual = a
            .fields
            .iter()
            .map(|f| f.polychromatic_rms_um)
            .collect::<Option<Vec<_>>>()
            .and_then(|v| v.into_iter().reduce(f64::max));
        push(
            "最差视场 RMS µm".into(),
            actual,
            limit,
            "max",
            "约束最差视场，不使用平均 RMS 替代。",
        );
    }
    if g.require_sensor_coverage {
        for index in 0..3 {
            let point = a.coverage.as_ref().and_then(|v| v.checkpoints.get(index));
            let passed = point.is_some_and(|v| v.status == "sampled_clear");
            push(
                format!("传感器覆盖检查点 {}", index + 1),
                Some(if passed { 1. } else { 0. }),
                1.,
                "min",
                "必须在实际像高中验证短边、长边、角落，且全部采样光线通过。MTF 另行验收。",
            );
        }
    }
    out
}
pub fn penalties(a: &Analysis, before: &Analysis, g: &QualityGuards) -> Vec<f64> {
    checks(a, before, g)
        .iter()
        .map(|c| {
            c.actual.map_or(1e6, |v| {
                if c.relation == "min" {
                    (c.limit - v).max(0.)
                } else {
                    (v - c.limit).max(0.)
                }
            }) / c.limit.abs().max(1e-3)
                * 100.
        })
        .collect()
}
