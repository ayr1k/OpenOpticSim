import { validTask } from "./designTask";
import { validCheckpoint } from "./searchCheckpoint";
import { validMotion } from "./motion";
import { validCoating } from "./coatings";
import type { Project } from "./types";
import { optimizationError } from "./optimization";
import { validAperture, validGroups, validScan } from "./design";
import { validFieldSpec, MAX_FIELD_RAD } from "./fields";
export const copy = <T>(v: T): T => structuredClone(v);
export function projectFromJson(text: string, nested = false): Project {
  if (new TextEncoder().encode(text).length > 4_000_000)
    throw Error("项目文件不能超过 4 MB");
  const p = JSON.parse(text);
  if (p?.workspace?.search_checkpoint !== undefined) {
    const checkpoint = p.workspace.search_checkpoint;
    if (nested || !validCheckpoint(checkpoint))
      throw Error("搜索断点格式无效或版本不兼容");
    for (const candidate of checkpoint.candidates) {
      if (candidate.project?.workspace)
        throw Error("搜索断点候选不能嵌套工作区");
      projectFromJson(JSON.stringify(candidate.project), true);
    }
  }
  if (
    ![1, 2, 3].includes(p?.schema_version) ||
    !Array.isArray(p.surfaces) ||
    !Array.isArray(p.materials) ||
    !Array.isArray(p.fields) ||
    !Array.isArray(p.wavelengths) ||
    !Array.isArray(p.sensor) ||
    typeof p.name !== "string"
  )
    throw Error("不是有效的 v1/v2/v3 OpenOpticSim 项目");
  if (
    p.surfaces.length > 100 ||
    !p.materials.length ||
    p.materials.length > 1000 ||
    !Number.isInteger(p.stop) ||
    p.stop < 0 ||
    (p.surfaces.length ? p.stop >= p.surfaces.length : p.stop !== 0) ||
    p.sensor.length !== 2 ||
    p.sensor.some(
      (v: unknown) => typeof v !== "number" || !Number.isFinite(v) || v <= 0,
    ) ||
    typeof p.description !== "string" ||
    (p.object_distance !== null &&
      (typeof p.object_distance !== "number" ||
        !Number.isFinite(p.object_distance))) ||
    !p.fields.length ||
    p.fields.length > 15 ||
    p.fields.some(
      (v: unknown) =>
        typeof v !== "number" ||
        !Number.isFinite(v) ||
        Math.abs(v) > MAX_FIELD_RAD,
    ) ||
    !p.wavelengths.length ||
    p.wavelengths.length > 15 ||
    p.wavelengths.some(
      (w: { nm: number; weight: number }) =>
        !w ||
        !Number.isFinite(w.nm) ||
        !Number.isFinite(w.weight) ||
        w.weight <= 0,
    )
  )
    throw Error("项目结构或设置无效");
  if (!validFieldSpec(p.field_spec)) throw Error("视场定义无效");
  if (
    p.field_spec &&
    (p.schema_version < 3 ||
      p.fields.length !==
        (p.field_spec.mode === "sensor" ? 4 : p.field_spec.heights_mm.length))
  )
    throw Error("目标像高视场需要 v3 项目，且视场数量必须一致");
  for (const s of p.surfaces) {
    if (s.coating !== undefined && !validCoating(s.coating))
      throw Error("镀膜数据无效：最多 32 层，折射率 1–10、物理厚度 0–10000 nm");
    if (
      !s ||
      typeof s.name !== "string" ||
      ![s.curvature, s.conic, s.thickness, s.semi_aperture].every(
        (v) => typeof v === "number" && Number.isFinite(v),
      ) ||
      !Number.isInteger(s.material) ||
      s.material < 0 ||
      s.material >= p.materials.length ||
      !Array.isArray(s.asphere) ||
      s.asphere.length !== 4 ||
      !s.asphere.every(
        (v: unknown) => typeof v === "number" && Number.isFinite(v),
      )
    )
      throw Error("表面数据不完整");
    if (
      s.mechanical != null &&
      (p.schema_version < 2 ||
        !Number.isFinite(s.mechanical.semi_diameter) ||
        !["flat", "curved"].includes(s.mechanical.shape))
    )
      throw Error("机械边缘数据无效");
  }
  for (const m of p.materials) {
    if (
      !m ||
      typeof m.name !== "string" ||
      typeof m.source !== "string" ||
      !Number.isFinite(m.n) ||
      !Array.isArray(m.range_nm) ||
      m.range_nm.length !== 2 ||
      !m.range_nm.every(Number.isFinite) ||
      (m.sellmeier !== null &&
        (!Array.isArray(m.sellmeier) ||
          m.sellmeier.length !== 2 ||
          m.sellmeier.some(
            (r: unknown) =>
              !Array.isArray(r) || r.length !== 3 || !r.every(Number.isFinite),
          )))
    )
      throw Error("材料数据不完整");
  }
  if (p.workspace !== undefined) {
    const w = p.workspace;
    if (
      nested ||
      !w ||
      w.schema_version !== 1 ||
      !Array.isArray(w.versions) ||
      w.versions.length > 20
    )
      throw Error("版本工作区无效或含嵌套历史");
    if (w.motion !== undefined) {
      if (!validMotion(w.motion)) throw Error("多配置镜组数据无效");
      projectFromJson(JSON.stringify(w.motion.reference), true);
      if (w.motion.study?.saved)
        projectFromJson(
          JSON.stringify(w.motion.study.saved.result.reference),
          true,
        );
    }
    if (w.design_task !== undefined && !validTask(w.design_task))
      throw Error("保存的设计任务无效");
    const ids = new Set<string>();
    if (w.groups !== undefined && !validGroups(p, w.groups))
      throw Error("保存的镜组范围无效或重叠");
    if (w.aperture !== undefined && !validAperture(w.aperture))
      throw Error("保存的光圈扫描设置无效");
    if (w.aperture_scan !== undefined) {
      if (!validScan(w.aperture_scan)) throw Error("保存的光圈扫描结果无效");
      projectFromJson(JSON.stringify(w.aperture_scan.source), true);
    }
    for (const v of w.versions) {
      if (
        !v ||
        typeof v.id !== "string" ||
        !v.id ||
        v.id.length > 120 ||
        ids.has(v.id) ||
        typeof v.name !== "string" ||
        !v.name.trim() ||
        v.name.length > 120 ||
        typeof v.created_at !== "string" ||
        !Number.isFinite(Date.parse(v.created_at)) ||
        typeof v.note !== "string" ||
        v.note.length > 1000
      )
        throw Error("版本记录无效");
      ids.add(v.id);
      projectFromJson(JSON.stringify(v.prescription), true);
      if (v.motion !== undefined) {
        if (!validMotion(v.motion)) throw Error("版本的多配置镜组数据无效");
        projectFromJson(JSON.stringify(v.motion.reference), true);
        if (v.motion.study?.saved)
          projectFromJson(
            JSON.stringify(v.motion.study.saved.result.reference),
            true,
          );
      }
      if (v.groups !== undefined && !validGroups(v.prescription, v.groups))
        throw Error("版本的镜组范围无效");
      if (
        v.analysis_preset !== undefined &&
        !["center", "balanced", "edge"].includes(v.analysis_preset)
      )
        throw Error("版本的分析预设无效");
    }
    if (w.focus !== undefined) {
      const f = w.focus;
      if (
        !f ||
        ![f.min_mm, f.max_mm, f.steps, f.sampling].every(Number.isFinite) ||
        f.min_mm < 0.01 ||
        f.max_mm <= f.min_mm ||
        f.max_mm > 1e5 ||
        !Number.isInteger(f.steps) ||
        f.steps < 3 ||
        f.steps > 101 ||
        !Number.isInteger(f.sampling) ||
        f.sampling < 7 ||
        f.sampling > 33 ||
        f.sampling % 2 !== 1 ||
        !["center", "balanced"].includes(f.criterion)
      )
        throw Error("保存的对焦扫描设置无效");
    }
    if (w.constraints !== undefined) {
      const c = w.constraints;
      if (
        !c ||
        ![
          c.target_efl_mm,
          c.efl_tolerance_mm,
          c.max_length_mm,
          c.min_center_mm,
          c.min_edge_mm,
          c.min_air_mm,
        ].every(Number.isFinite) ||
        c.target_efl_mm <= 0 ||
        c.efl_tolerance_mm <= 0 ||
        c.max_length_mm <= 0 ||
        c.min_center_mm < 0 ||
        c.min_edge_mm < 0 ||
        c.min_air_mm < 0
      )
        throw Error("保存的优化约束无效");
      if (c.design !== undefined) {
        const g = c.design;
        if (
          !g ||
          ![
            g.f_number,
            g.f_tolerance,
            g.min_bfl_mm,
            g.max_front_diameter_mm,
          ].every(Number.isFinite) ||
          g.f_number < 0.5 ||
          g.f_number > 128 ||
          g.f_tolerance <= 0 ||
          g.min_bfl_mm < 0 ||
          g.max_front_diameter_mm <= 0
        )
          throw Error("保存的设计目标无效");
      }
    }
    if (
      w.optimization !== undefined &&
      optimizationError(p, w.optimization, false)
    )
      throw Error("保存的优化变量或预设无效");
  }
  return p;
}
const header =
  "name,curvature,conic,a4,a6,a8,a10,thickness,semi_aperture,material";
const headerV2 = header + ",mechanical_semi_diameter,edge_shape";
const quote = (s: string) => '"' + s.replaceAll('"', '""') + '"';
export function toCsv(p: Project): string {
  const { surfaces, ...meta } = p;
  return (
    "#oos=" +
    JSON.stringify(meta) +
    "\n" +
    (p.schema_version >= 2 ? headerV2 : header) +
    "\n" +
    surfaces
      .map((s) =>
        [
          quote(s.name),
          s.curvature,
          s.conic,
          ...s.asphere,
          s.thickness,
          s.semi_aperture,
          s.material,
          ...(p.schema_version >= 2
            ? [s.mechanical?.semi_diameter ?? "", s.mechanical?.shape ?? ""]
            : []),
        ].join(","),
      )
      .join("\n")
  );
}
function rows(text: string): string[][] {
  const out: string[][] = [];
  let row: string[] = [],
    cell = "",
    quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '"') {
      if (quoted && text[i + 1] === '"') {
        cell += '"';
        i++;
      } else quoted = !quoted;
    } else if (c === "," && !quoted) {
      row.push(cell);
      cell = "";
    } else if (c === "\n" && !quoted) {
      row.push(cell.replace(/\r$/, ""));
      out.push(row);
      row = [];
      cell = "";
    } else cell += c;
  }
  if (quoted) throw Error("CSV 引号未闭合");
  if (cell || row.length) {
    row.push(cell);
    out.push(row);
  }
  return out;
}
export function fromCsv(text: string): Project {
  const line = text.indexOf("\n");
  if (!text.startsWith("#oos=") || line < 0)
    throw Error(
      "CSV 必须包含 OSS 材料与项目元数据；请使用“导出处方 CSV”的格式",
    );
  const meta = JSON.parse(text.slice(5, line));
  const data = rows(text.slice(line + 1));
  const columns = data.shift()?.join(",");
  if (columns !== header && columns !== headerV2)
    throw Error("CSV 列格式不匹配");
  const extended = columns === headerV2;
  const surfaces = data
    .filter((r) => r.length > 1)
    .map((r) => {
      if (r.length !== (extended ? 12 : 10)) throw Error("CSV 列数不匹配");
      if (r.slice(1, 10).some((v) => !v.trim()))
        throw Error("CSV 数值不能为空");
      const n = r.slice(1, 10).map(Number);
      if (n.some((x) => !Number.isFinite(x))) throw Error("CSV 包含非数值参数");
      return {
        name: r[0],
        curvature: n[0],
        conic: n[1],
        asphere: n.slice(2, 6),
        thickness: n[6],
        semi_aperture: n[7],
        material: n[8],
        ...(extended && (r[10] || r[11])
          ? { mechanical: { semi_diameter: Number(r[10]), shape: r[11] } }
          : {}),
      };
    });
  return projectFromJson(JSON.stringify({ ...meta, surfaces }));
}
export function download(
  name: string,
  content: string,
  type = "application/json",
) {
  const url = URL.createObjectURL(new Blob([content], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
export function sag(s: Project["surfaces"][number], r: number) {
  const q = 1 - (1 + s.conic) * s.curvature * s.curvature * r * r;
  if (q <= 1e-14) return NaN;
  return (
    (s.curvature * r * r) / (1 + Math.sqrt(q)) +
    s.asphere.reduce((z, a, i) => z + a * r ** (4 + 2 * i), 0)
  );
}
export function outerRadius(s: Project["surfaces"][number]) {
  return s.mechanical?.semi_diameter ?? s.semi_aperture;
}
export function profile(s: Project["surfaces"][number], r: number) {
  return sag(
    s,
    s.mechanical?.shape === "flat" ? Math.min(r, s.semi_aperture) : r,
  );
}
// Include the exact annulus boundary so the corner survives rendering at any scale.
export function profileHeights(s: Project["surfaces"][number]) {
  const r = outerRadius(s);
  return [
    ...new Set([
      ...Array.from({ length: 65 }, (_, j) => -r + (2 * r * j) / 64),
      -s.semi_aperture,
      s.semi_aperture,
    ]),
  ].sort((a, b) => a - b);
}
