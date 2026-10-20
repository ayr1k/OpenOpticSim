import catalog from "./data/glass-catalog.json";
import type { Material, Project } from "./types";
interface CatalogGlass extends Material {
  nd_reference: number;
  vd_reference: number;
  url: string;
  sha256: string;
}
export const glasses: CatalogGlass[] = catalog.entries.map((m) => ({
  ...m,
  range_nm: [m.range_nm[0], m.range_nm[1]],
}));
export function refractiveIndex(m: Material, nm: number) {
  if (nm < m.range_nm[0] || nm > m.range_nm[1]) return null;
  if (!m.sellmeier) return m.n;
  const l2 = (nm / 1000) ** 2;
  const n2 =
    1 +
    m.sellmeier[0].reduce(
      (v, b, i) => v + (b * l2) / (l2 - m.sellmeier![1][i]),
      0,
    );
  return n2 > 0 && Number.isFinite(n2) ? Math.sqrt(n2) : null;
}
export function glassNumbers(m: Material) {
  const d = refractiveIndex(m, 587.5618),
    f = refractiveIndex(m, 486.1327),
    c = refractiveIndex(m, 656.2725),
    g = refractiveIndex(m, 435.8343);
  return {
    nd: d,
    pgf:
      g !== null && f !== null && c !== null && f !== c
        ? (g - f) / (f - c)
        : null,
    vd:
      d !== null && f !== null && c !== null && f !== c
        ? (d - 1) / (f - c)
        : null,
  };
}
export function glassDescription(m: Material) {
  const { nd, vd } = glassNumbers(m);
  if (nd === null || vd === null) return "当前模型无法计算可见光色散参数";
  return `${vd >= 60 ? "低色散冠玻璃" : vd <= 35 ? "高色散火石玻璃" : "中等色散玻璃"}；${nd >= 1.7 ? "高折射率可在相同光焦度下降低曲率，但并不自动消除像差" : "需结合曲率、厚度与另一种玻璃共同校正色差"}。Vd 越大，可见光主色散通常越小。`;
}
export function achromatPowers(focal: number, crownV: number, flintV: number) {
  if (
    ![focal, crownV, flintV].every(Number.isFinite) ||
    focal <= 0 ||
    crownV <= flintV ||
    flintV <= 0
  )
    return null;
  return {
    positive: crownV / (focal * (crownV - flintV)),
    negative: -flintV / (focal * (crownV - flintV)),
  };
}
export function applyGlass(p: Project, surface: number, m: Material) {
  if (
    surface < 0 ||
    surface >= p.surfaces.length - 1 ||
    p.surfaces[surface].material === 0
  )
    throw Error("请选择玻璃实体所在的面后材料，不能替换空气或光阑。");
  if (p.wavelengths.some((w) => refractiveIndex(m, w.nm) === null))
    throw Error("当前波长超出材料有效范围。");
  const snapshot: Material = {
    name: m.name,
    n: m.n,
    sellmeier: structuredClone(m.sellmeier),
    range_nm: [...m.range_nm],
    source: m.source,
  };
  let index = p.materials.findIndex(
    (existing) => JSON.stringify(existing) === JSON.stringify(snapshot),
  );
  if (index < 0) {
    if (p.materials.length >= 1000) throw Error("材料数量已达上限");
    index = p.materials.push(snapshot) - 1;
  }
  p.surfaces[surface].material = index;
}
