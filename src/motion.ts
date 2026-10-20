import type {
  MotionConfiguration,
  MotionOptions,
  MotionResult,
  MotionWorkspace,
  Project,
} from "./types";
import { opticalKey, validGroups } from "./design";
import { prescription } from "./versions";
import { validStudy } from "./study";

export function createMotion(p: Project): MotionWorkspace {
  const groups = p.workspace?.groups ?? [];
  if (!groups.length) throw Error("请先在镜组管理中创建一个或多个完整镜组");
  return {
    schema_version: 1,
    reference: prescription(p),
    groups: groups.map((g) => ({ ...g, min_mm: -5, max_mm: 5, locked: false })),
    configurations: [
      {
        id: crypto.randomUUID(),
        name: "参考配置",
        positions_mm: groups.map(() => 0),
        object_distance: p.object_distance,
        target_efl_mm: null,
        target_f_number: null,
      },
    ],
    active_id: null,
    min_air_mm: 0.05,
    sampling: 17,
    criterion: "balanced",
    max_rms_um: 30,
  };
}
export function motionOptions(
  m: MotionWorkspace,
  c: MotionConfiguration,
  solve: number | null = null,
): MotionOptions {
  return {
    groups: m.groups,
    positions_mm: c.positions_mm,
    object_distance: c.object_distance,
    min_air_mm: m.min_air_mm,
    target_f_number: c.target_f_number,
    solve_group: solve,
    sampling: m.sampling,
    criterion: m.criterion,
    max_rms_um: m.max_rms_um,
  };
}
export function validMotion(m: MotionWorkspace): boolean {
  if (
    !m ||
    !validStudy(m.study) ||
    m.schema_version !== 1 ||
    !m.reference ||
    "workspace" in m.reference ||
    !validGroups(m.reference, m.groups) ||
    !m.groups.length ||
    !Array.isArray(m.configurations) ||
    !m.configurations.length ||
    m.configurations.length > 30 ||
    !Number.isFinite(m.min_air_mm) ||
    m.min_air_mm < 0 ||
    !Number.isFinite(m.max_rms_um) ||
    m.max_rms_um <= 0 ||
    !Number.isInteger(m.sampling) ||
    m.sampling < 7 ||
    m.sampling > 33 ||
    m.sampling % 2 !== 1 ||
    !["center", "balanced"].includes(m.criterion)
  )
    return false;
  if (
    m.groups.some(
      (g) =>
        ![g.min_mm, g.max_mm].every(Number.isFinite) ||
        g.min_mm > g.max_mm ||
        Math.abs(g.min_mm) > 10000 ||
        Math.abs(g.max_mm) > 10000 ||
        typeof g.locked !== "boolean",
    )
  )
    return false;
  const ids = new Set<string>();
  for (const c of m.configurations) {
    if (
      !c ||
      typeof c.id !== "string" ||
      !c.id ||
      c.id.length > 120 ||
      ids.has(c.id) ||
      typeof c.name !== "string" ||
      !c.name.trim() ||
      c.name.length > 120 ||
      !Array.isArray(c.positions_mm) ||
      c.positions_mm.length !== m.groups.length ||
      !c.positions_mm.every(Number.isFinite) ||
      (c.object_distance !== null &&
        (!Number.isFinite(c.object_distance) || c.object_distance <= 1)) ||
      (c.target_efl_mm !== null &&
        (!Number.isFinite(c.target_efl_mm) || c.target_efl_mm <= 0)) ||
      (c.target_f_number !== null &&
        (!Number.isFinite(c.target_f_number) ||
          c.target_f_number < 0.5 ||
          c.target_f_number > 128))
    )
      return false;
    ids.add(c.id);
  }
  return m.active_id === null || ids.has(m.active_id);
}
export function interpolate(
  a: MotionConfiguration,
  b: MotionConfiguration,
  t: number,
): MotionConfiguration {
  if (
    !Number.isFinite(t) ||
    t < 0 ||
    t > 1 ||
    a.positions_mm.length !== b.positions_mm.length ||
    a.object_distance !== b.object_distance ||
    a.target_f_number !== b.target_f_number
  )
    throw Error("插值需要相同物距、光圈规则和镜组数量");
  return {
    ...a,
    id: "interpolated",
    name: `${a.name} → ${b.name} ${(t * 100).toFixed(0)}%`,
    positions_mm: a.positions_mm.map((d, i) => d + (b.positions_mm[i] - d) * t),
    target_efl_mm:
      a.target_efl_mm !== null && b.target_efl_mm !== null
        ? a.target_efl_mm + (b.target_efl_mm - a.target_efl_mm) * t
        : null,
  };
}
export function applyMotion(
  p: Project,
  result: MotionResult,
  id: string | null,
) {
  const workspace = p.workspace!;
  Object.assign(p, structuredClone(result.project));
  p.workspace = workspace;
  workspace.motion!.active_id = id;
  workspace.motion!.application_revision =
    (workspace.motion!.application_revision ?? 0) + 1;
}
// Rebase shared prescription edits by their delta; preserve configuration positions.
// Changing topology invalidates the motion frame rather than silently shifting indices.
export function synchronizeMotion(before: Project, after: Project) {
  const old = before.workspace?.motion,
    m = after.workspace?.motion;
  if (
    !old ||
    !m ||
    JSON.stringify(old) !== JSON.stringify(m) ||
    opticalKey(before) === opticalKey(after)
  )
    return;
  if (before.surfaces.length !== after.surfaces.length) {
    delete after.workspace!.motion;
    return;
  }
  const reference = prescription(after);
  reference.surfaces.forEach((s, i) => {
    s.thickness =
      old.reference.surfaces[i].thickness +
      s.thickness -
      before.surfaces[i].thickness;
    s.semi_aperture =
      old.reference.surfaces[i].semi_aperture +
      s.semi_aperture -
      before.surfaces[i].semi_aperture;
  });
  if (JSON.stringify(before.fields) === JSON.stringify(after.fields)) {
    reference.fields = [...old.reference.fields];
  } else if (after.object_distance !== null && !after.field_spec) {
    const origin =
      old.reference.surfaces.reduce((z, s) => z + s.thickness, 0) -
      before.surfaces.reduce((z, s) => z + s.thickness, 0);
    const referenceDistance = after.object_distance - origin;
    reference.fields = after.fields.map((f) =>
      Math.atan((after.object_distance! * Math.tan(f)) / referenceDistance),
    );
  }
  if (before.object_distance !== after.object_distance && old.active_id) {
    const configuration = m.configurations.find((c) => c.id === old.active_id);
    const origin =
      old.reference.surfaces.reduce((z, s) => z + s.thickness, 0) -
      before.surfaces.reduce((z, s) => z + s.thickness, 0);
    if (configuration)
      configuration.object_distance =
        after.object_distance === null ? null : after.object_distance - origin;
  }
  reference.object_distance = old.reference.object_distance;
  m.reference = reference;
  m.active_id = null;
  if (!validGroups(reference, m.groups)) delete after.workspace!.motion;
}
