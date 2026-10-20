use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub n: f64,
    pub sellmeier: Option<[[f64; 3]; 2]>,
    pub range_nm: [f64; 2],
    pub source: String,
}
impl Material {
    pub fn air() -> Self {
        Self {
            name: "Air".into(),
            n: 1.,
            sellmeier: None,
            range_nm: [380., 780.],
            source: "Ideal vacuum approximation n=1".into(),
        }
    }
    pub fn index(&self, nm: f64) -> Result<f64, String> {
        if !nm.is_finite() || nm < self.range_nm[0] || nm > self.range_nm[1] {
            return Err(format!("{}: wavelength outside material range", self.name));
        }
        let n = if let Some([b, c]) = self.sellmeier {
            let l2 = (nm / 1000.).powi(2);
            (1. + (0..3).map(|i| b[i] * l2 / (l2 - c[i])).sum::<f64>()).sqrt()
        } else {
            self.n
        };
        if !n.is_finite() || n < 1. {
            Err(format!("{}: invalid refractive index", self.name))
        } else {
            Ok(n)
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeShape {
    Flat,
    Curved,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MechanicalEdge {
    pub semi_diameter: f64,
    pub shape: EdgeShape,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Surface {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coating: Vec<crate::coating::Layer>,
    pub name: String,
    /// Curvature in 1/mm; positive centre of curvature is toward +z.
    pub curvature: f64,
    pub conic: f64,
    /// A4, A6, A8, A10; sag coefficients in mm^(1-power).
    pub asphere: [f64; 4],
    pub thickness: f64,
    pub semi_aperture: f64,
    pub material: usize,
    /// Non-imaging extension only. Never used by ray intersection or aperture clipping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mechanical: Option<MechanicalEdge>,
}
impl Surface {
    pub fn outer_radius(&self) -> f64 {
        self.mechanical
            .as_ref()
            .map_or(self.semi_aperture, |m| m.semi_diameter)
    }
    pub fn profile(&self, r: f64) -> Option<f64> {
        let r = if matches!(
            self.mechanical.as_ref().map(|m| &m.shape),
            Some(EdgeShape::Flat)
        ) {
            r.min(self.semi_aperture)
        } else {
            r
        };
        self.sag(r).map(|s| s.0)
    }
    pub fn sag(&self, r: f64) -> Option<(f64, f64)> {
        let c = self.curvature;
        let q = 1. - (1. + self.conic) * c * c * r * r;
        if q <= 1e-14 {
            return None;
        }
        let root = q.sqrt();
        let mut z = c * r * r / (1. + root);
        let mut dz = c * r / root;
        for (i, a) in self.asphere.iter().enumerate() {
            let p = 4 + 2 * i as i32;
            z += a * r.powi(p);
            dz += p as f64 * a * r.powi(p - 1);
        }
        if z.is_finite() && dz.is_finite() {
            Some((z, dz))
        } else {
            None
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wavelength {
    pub nm: f64,
    pub weight: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: u32,
    pub name: String,
    pub description: String,
    pub materials: Vec<Material>,
    pub surfaces: Vec<Surface>,
    pub stop: usize,
    /// null = infinity; positive distance from first vertex to object plane.
    pub object_distance: Option<f64>,
    pub sensor: [f64; 2],
    pub fields: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_spec: Option<crate::fields::FieldSpec>,
    pub wavelengths: Vec<Wavelength>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub surface: Option<usize>,
    pub message: String,
}
impl Project {
    pub fn vertices(&self) -> Vec<f64> {
        let mut z = 0.;
        self.surfaces
            .iter()
            .map(|s| {
                let v = z;
                z += s.thickness;
                v
            })
            .collect()
    }
    pub fn image_z(&self) -> f64 {
        self.surfaces.iter().map(|s| s.thickness).sum()
    }
    pub fn validate(&self) -> Vec<Diagnostic> {
        let mut d = self.validate_structure();
        if d.is_empty() {
            d.extend(crate::geometry::inspect(self).diagnostics);
        }
        d
    }
    pub fn validate_optical(&self) -> Vec<Diagnostic> {
        let mut d = self.validate();
        if self.surfaces.is_empty() {
            d.push(Diagnostic {
                surface: None,
                message: "Add a lens before optical analysis".into(),
            });
        }
        d
    }
    pub fn validate_structure(&self) -> Vec<Diagnostic> {
        let mut d = Vec::new();
        let mut add = |surface, message: &str| {
            d.push(Diagnostic {
                surface,
                message: message.into(),
            })
        };
        if ![1, 2, 3].contains(&self.schema_version) {
            add(None, "Unsupported project schema version");
        }
        if self.surfaces.len() > 100 {
            add(None, "Expected at most 100 surfaces");
        }
        if (!self.surfaces.is_empty() && self.stop >= self.surfaces.len())
            || (self.surfaces.is_empty() && self.stop != 0)
        {
            add(None, "Invalid stop index");
        }
        if self.materials.is_empty() {
            add(None, "Material catalog must contain air at index 0");
        }
        if self.fields.is_empty()
            || self.fields.len() > 15
            || self
                .fields
                .iter()
                .any(|f| !f.is_finite() || f.abs() > crate::fields::MAX_FIELD_RAD)
        {
            add(
                None,
                "Fields must contain 1..15 angles within ±0.7 rad (40.1°)",
            );
        }
        if let Some(crate::fields::FieldSpec::ImageHeights { heights_mm }) = &self.field_spec {
            if heights_mm.is_empty()
                || heights_mm.len() > 15
                || heights_mm
                    .iter()
                    .any(|h| !h.is_finite() || h.abs() > 10000.)
            {
                add(
                    None,
                    "Image heights must contain 1..15 finite values within ±10000 mm",
                );
            }
            if heights_mm.len() != self.fields.len() {
                add(None, "Field angle cache count must match target heights");
            }
        }
        if self.field_spec.is_some() && self.schema_version < 3 {
            add(None, "Target image-height fields require project schema 3");
        }
        if matches!(self.field_spec, Some(crate::fields::FieldSpec::Sensor))
            && self.fields.len() != 4
        {
            add(None, "Sensor field mode requires four field entries");
        }
        if self.wavelengths.is_empty()
            || self.wavelengths.len() > 15
            || self.wavelengths.iter().any(|w| {
                !w.nm.is_finite()
                    || w.nm < 380.
                    || w.nm > 780.
                    || !w.weight.is_finite()
                    || w.weight <= 0.
            })
        {
            add(
                None,
                "Expected 1..15 visible wavelengths with positive weights",
            );
        }
        if self
            .object_distance
            .is_some_and(|v| !v.is_finite() || v <= 1. || v > 1e12)
        {
            add(None, "Object distance must be within (1, 1e12] mm");
        }
        if self.sensor.iter().any(|v| !v.is_finite() || *v <= 0.) {
            add(None, "Invalid sensor size");
        }
        for material in &self.materials {
            if !material.n.is_finite()
                || material.n < 1.
                || material.range_nm.iter().any(|v| !v.is_finite())
                || material.range_nm[0] >= material.range_nm[1]
                || material
                    .sellmeier
                    .is_some_and(|a| a.iter().flatten().any(|v| !v.is_finite()))
            {
                add(None, "Invalid material coefficients or wavelength range");
            }
        }
        for (i, s) in self.surfaces.iter().enumerate() {
            if s.coating.iter().any(|l| l.index > 10.)
                || crate::coating::response(1., 1.5, 0., 550., &s.coating).is_err()
            {
                add(Some(i), "Invalid coating layers");
            }
            if !s.thickness.is_finite()
                || s.thickness < 0.
                || !s.semi_aperture.is_finite()
                || s.semi_aperture <= 0.
                || !s.curvature.is_finite()
                || !s.conic.is_finite()
                || s.asphere.iter().any(|a| !a.is_finite())
            {
                add(Some(i), "Invalid surface dimensions");
                continue;
            }
            if s.thickness > 1e6
                || s.semi_aperture > 1e4
                || s.curvature.abs() > 10.
                || s.conic.abs() > 1e6
                || s.asphere.iter().any(|a| a.abs() > 1.)
            {
                add(Some(i), "Surface outside supported numerical range");
                continue;
            }
            if s.material >= self.materials.len() {
                add(Some(i), "Unknown material");
                continue;
            }
            if s.sag(s.semi_aperture).is_none() {
                add(Some(i), "Surface sag is undefined at aperture edge");
            }
            if let Some(m) = &s.mechanical {
                if self.schema_version < 2 {
                    add(Some(i), "Mechanical edges require schema version 2");
                }
                if !m.semi_diameter.is_finite()
                    || m.semi_diameter < s.semi_aperture
                    || m.semi_diameter > 1e4
                {
                    add(
                        Some(i),
                        "Mechanical radius must cover the clear aperture and be at most 10000 mm",
                    );
                } else if s.profile(m.semi_diameter).is_none() {
                    add(Some(i), "Mechanical extension exceeds surface domain; use a flat annulus or reduce diameter");
                }
            }
            if let Some(next) = self.surfaces.get(i + 1) {
                if s.material != 0
                    && (s.mechanical.is_some() || next.mechanical.is_some())
                    && (s.outer_radius() - next.outer_radius()).abs() > 1e-9
                {
                    add(
                        Some(i),
                        "Lens mechanical radii must match; set the whole-lens diameter",
                    );
                }
            }
            if self
                .wavelengths
                .iter()
                .any(|w| self.materials[s.material].index(w.nm).is_err())
            {
                add(Some(i), "Material invalid at selected wavelengths");
            }
        }
        if self.surfaces.last().is_some_and(|s| s.thickness <= 0.) {
            add(None, "Image plane must follow the last surface");
        }
        if let (Some(distance), Some(first)) = (self.object_distance, self.surfaces.first()) {
            let min_sag = (0..=32)
                .filter_map(|i| first.sag(first.semi_aperture * i as f64 / 32.).map(|v| v.0))
                .fold(0., f64::min);
            if -distance >= min_sag - 0.5 {
                add(None, "Object plane must precede the entire first surface");
            }
        }
        d
    }
}
pub fn examples() -> Vec<Project> {
    let air = Material::air();
    // Deliberately synthetic glass models and original teaching prescriptions.
    let glass = Material {
        name: "Teaching crown".into(),
        n: 1.52,
        sellmeier: Some([[1.04, 0.23, 1.01], [0.006, 0.02, 103.]]),
        range_nm: [380., 780.],
        source: "OSS synthetic teaching material; not a commercial glass".into(),
    };
    let flint = Material {
        name: "Teaching flint".into(),
        n: 1.62,
        sellmeier: Some([[1.30, 0.30, 1.10], [0.009, 0.04, 110.]]),
        range_nm: [380., 780.],
        source: "OSS synthetic teaching material; not a commercial glass".into(),
    };
    let s = |name: &str, r: f64, t: f64, a: f64, m: usize| Surface {
        name: name.into(),
        curvature: if r == 0. { 0. } else { 1. / r },
        conic: 0.,
        asphere: [0.; 4],
        thickness: t,
        semi_aperture: a,
        material: m,
        mechanical: None,
        coating: vec![],
    };
    let base = Project {
        schema_version: 1,
        name: "01 · 单透镜".into(),
        description:
            "原创教学处方。缩小光阑，观察球差与通光的变化。材料为合成教学模型，不代表商品镜头。"
                .into(),
        materials: vec![air, glass, flint],
        surfaces: vec![
            s("光阑", 0., 2., 6., 0),
            s("前表面", 50., 4., 12., 1),
            s("后表面", -50., 46., 12., 0),
        ],
        stop: 0,
        object_distance: None,
        sensor: [36., 24.],
        fields: vec![0., 0.07, 0.1],
        field_spec: None,
        wavelengths: vec![
            Wavelength {
                nm: 486.1327,
                weight: 1.,
            },
            Wavelength {
                nm: 587.5618,
                weight: 1.,
            },
            Wavelength {
                nm: 656.2725,
                weight: 1.,
            },
        ],
    };
    let mut doublet = base.clone();
    doublet.name = "02 · 双胶合探索".into();
    doublet.surfaces = vec![
        s("光阑", 0., 2., 5., 0),
        s("冠玻璃前面", 40., 5., 10., 1),
        s("胶合面", -30., 2., 10., 2),
        s("火石玻璃后面", -100., 65., 10., 0),
    ];
    let mut triplet = base.clone();
    triplet.name = "03 · 三片式探索".into();
    triplet.stop = 2;
    triplet.surfaces = vec![
        s("第一片前面", 40., 4., 10., 1),
        s("第一片后面", -70., 4., 10., 0),
        s("光阑", 0., 2., 4., 0),
        s("第二片前面", -35., 2., 8., 2),
        s("第二片后面", 35., 4., 8., 0),
        s("第三片前面", 60., 4., 10., 1),
        s("第三片后面", -40., 55., 10., 0),
    ];
    let mut gauss = base.clone();
    gauss.name = "04 · 双高斯结构探索".into();
    gauss.stop = 6;
    gauss.surfaces = vec![
        s("前组外片前面", 70., 4., 12., 1),
        s("前组外片后面", -200., 2., 12., 0),
        s("前组内片前面", 45., 4., 11., 1),
        s("前组内片后面", 140., 2., 11., 0),
        s("前组负片前面", 100., 2., 9., 2),
        s("前组负片后面", 35., 4., 9., 0),
        s("光阑", 0., 4., 4., 0),
        s("后组负片前面", -35., 2., 9., 2),
        s("后组负片后面", -100., 2., 9., 0),
        s("后组内片前面", -140., 4., 11., 1),
        s("后组内片后面", -45., 2., 11., 0),
        s("后组外片前面", 200., 4., 12., 1),
        s("后组外片后面", -70., 60., 12., 0),
    ];
    let mut annulus = base.clone();
    annulus.schema_version = 2;
    annulus.name = "05 · 机械环带探索".into();
    annulus.description = "原创弯月教学镜片。机械直径 26 mm，前后有效直径分别为 16 / 12 mm。改变机械外径观察轮廓；光学参数不变时，成像结果应保持一致。".into();
    annulus.surfaces = vec![
        s("光阑", 0., 2., 4., 0),
        s("方边弯月镜片 · 前面", 12.5, 5., 8., 1),
        s("方边弯月镜片 · 后面", 20., 45., 6., 0),
    ];
    for face in annulus.surfaces.iter_mut().skip(1) {
        face.mechanical = Some(MechanicalEdge {
            semi_diameter: 13.,
            shape: EdgeShape::Flat,
        });
    }
    vec![base, doublet, triplet, gauss, annulus]
}
