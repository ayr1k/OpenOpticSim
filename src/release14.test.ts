import { expect, it } from "vitest";
import {
  defaultTask,
  applyTask,
  validTask,
  projectFromCandidate,
} from "./designTask";
import { projectFromJson } from "./project";
import type { Project } from "./types";
import example from "../examples/tessar-50mm-1.3.oos.json";
import { TaskQueue } from "./taskQueue";
import { mtfAt } from "./qualityReport";
it("persists a task and binds coverage, sensor and worst-field guard without changing surfaces", () => {
  const p = structuredClone(example) as Project,
    surfaces = structuredClone(p.surfaces);
  applyTask(p, { ...defaultTask, sensor_mm: [6, 4] });
  const reopened = projectFromJson(JSON.stringify(p));
  expect(reopened).toEqual(p);
  expect(p.surfaces).toEqual(surfaces);
  expect(p.field_spec).toEqual({ mode: "sensor" });
  expect(p.fields).toHaveLength(4);
  expect(p.workspace?.optimization?.quality?.require_sensor_coverage).toBe(
    true,
  );
  expect(p.workspace?.constraints?.target_efl_mm).toBe(50);
  expect(validTask({ ...defaultTask, starts: 4 })).toBe(false);
  p.workspace!.design_task!.sensor_mm[0] = -1;
  expect(() => projectFromJson(JSON.stringify(p))).toThrow("设计任务");
});
it("queues, cancels waiting work and resumes without launching cancelled work", async () => {
  const q = new TaskQueue();
  q.setPaused(true);
  let executed = 0;
  const controller = new AbortController();
  const cancelled = q.run("cancel", controller.signal, async () => {
    executed++;
    return 1;
  });
  const rejection = expect(cancelled).rejects.toThrow("Cancelled");
  controller.abort();
  await rejection;
  const good = q.run("good", new AbortController().signal, async () => {
    executed++;
    return 2;
  });
  expect(executed).toBe(0);
  q.setPaused(false);
  expect(await good).toBe(2);
  expect(executed).toBe(1);
  expect(q.snapshot().map((r) => r.state)).toEqual(["cancelled", "done"]);
});
it("a cancelled running task keeps its slot until its worker exits", async () => {
  const q = new TaskQueue();
  let release!: () => void,
    second = false;
  const pending = q.run(
    "slow",
    new AbortController().signal,
    async () =>
      new Promise<void>((r) => {
        release = r;
      }),
  );
  const rejection = expect(pending).rejects.toThrow("Cancelled");
  await Promise.resolve();
  q.cancel(q.snapshot()[0].id);
  const next = q.run("next", new AbortController().signal, async () => {
    second = true;
  });
  await Promise.resolve();
  expect(second).toBe(false);
  release();
  await rejection;
  await next;
  expect(second).toBe(true);
});
it("does not extrapolate MTF beyond the sampled frequency range", () => {
  expect(mtfAt([1, 0.8, 0.4], 10, 15)).toBeCloseTo(0.6);
  expect(mtfAt([1, 0.8, 0.4], 10, 30)).toBeNull();
  expect(mtfAt([1], 0, 10)).toBeNull();
});
it("adoption retains verified geometry and the generation thickness thresholds", () => {
  const candidate = {
    project: structuredClone(example) as Project,
    accepted: true,
    before_rms_um: 2,
    after_rms_um: 1,
    iterations: 1,
    note: "",
    sampling: 33,
    checks: [
      {
        name: "center_mm",
        surface: 0,
        actual: 1,
        limit: 0.25,
        relation: "min" as const,
        passed: true,
      },
      {
        name: "body_min_mm",
        surface: 0,
        actual: 0.3,
        limit: 0.05,
        relation: "min" as const,
        passed: true,
      },
    ],
  };
  const p = projectFromCandidate(candidate, defaultTask);
  expect(p.surfaces).toEqual(candidate.project.surfaces);
  expect(p.fields).toEqual(candidate.project.fields);
  expect(p.workspace?.constraints?.min_center_mm).toBe(0.25);
  expect(p.workspace?.constraints?.min_edge_mm).toBe(0.05);
  expect(projectFromJson(JSON.stringify(p))).toEqual(p);
  expect(() =>
    projectFromCandidate({ ...candidate, accepted: false }, defaultTask),
  ).toThrow("验收");
});
