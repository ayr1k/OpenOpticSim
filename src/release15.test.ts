import { expect, it } from "vitest";
import { defaultTask, applyTask, validTask } from "./designTask";
import { projectFromJson } from "./project";
import example from "../examples/24-70mm-zoom.oos.json";
it("roundtrips an interrupted search without changing zoom prescription or motion", () => {
  const p = projectFromJson(JSON.stringify(example));
  const surfaces = structuredClone(p.surfaces),
    motion = structuredClone(p.workspace?.motion);
  applyTask(p, {
    ...defaultTask,
    min_center_mm: 1.2,
    min_edge_mm: 0.3,
    exploration: "expanded",
    element_glass: true,
  });
  p.workspace!.search_checkpoint = {
    engine_version: "1.5.0-search1",
    brief: p.workspace!.design_task!,
    cursor: 1,
    total: 4,
    candidates: [],
    attempts: ["第一尝试未通过"],
  };
  const reopened = projectFromJson(JSON.stringify(p));
  expect(reopened).toEqual(p);
  expect(reopened.surfaces).toEqual(surfaces);
  expect(reopened.workspace?.motion).toEqual(motion);
  expect(reopened.workspace?.constraints?.min_center_mm).toBe(1.2);
  p.workspace!.search_checkpoint.cursor = 5;
  expect(() => projectFromJson(JSON.stringify(p))).toThrow("断点");
});
it("rejects invalid search budgets and thickness requirements", () => {
  expect(validTask({ ...defaultTask, max_attempts: 0 })).toBe(false);
  expect(validTask({ ...defaultTask, max_attempts: 241 })).toBe(false);
  expect(validTask({ ...defaultTask, min_edge_mm: 2, min_center_mm: 1 })).toBe(
    false,
  );
  expect(validTask({ ...defaultTask, min_center_mm: NaN })).toBe(false);
  expect(
    validTask({
      ...defaultTask,
      exploration: undefined,
      max_attempts: undefined,
    }),
  ).toBe(true);
});
