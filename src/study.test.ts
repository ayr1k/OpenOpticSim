import { expect, it } from "vitest";
import zoom from "../examples/24-70mm-zoom.oos.json";
import { projectFromJson } from "./project";
import {
  defaultStudy,
  studyOptions,
  studyKey,
  validStudy,
  studyCsv,
} from "./study";
import { addVersion, restoreVersion } from "./versions";
import type { StudyResult } from "./types";
import resultFixture from "../docs/fixtures/study-1.2-results.json";
const load = () => projectFromJson(JSON.stringify(zoom));
it("restores complete scan records and rejects corrupted archived prescriptions", () => {
  const p = load(),
    m = p.workspace!.motion!;
  m.study = {
    settings: defaultStudy(m),
    saved: {
      source_key: studyKey(m),
      settings: defaultStudy(m),
      result: resultFixture as unknown as StudyResult,
    },
  };
  const saved = projectFromJson(JSON.stringify(p));
  expect(saved).toEqual(p);
  expect(saved.workspace!.motion!.study!.saved!.result.points).toHaveLength(13);
  const broken = structuredClone(p);
  broken.workspace!.motion!.study!.saved!.result.reference.sensor = [-1, 3];
  expect(() => projectFromJson(JSON.stringify(broken))).toThrow();
});
it("round trips study settings and named versions without changing optics", () => {
  const p = load(),
    m = p.workspace!.motion!,
    before = studyKey(m);
  m.study = { settings: defaultStudy(m) };
  m.study.settings.variables = [
    { kind: "curvature", surface: 0, min: 0, max: 0.1 },
  ];
  const v = addVersion(p, p, "study");
  const q = projectFromJson(JSON.stringify(p));
  expect(q).toEqual(p);
  expect(studyKey(m)).toBe(before);
  expect(
    restoreVersion(q, v.prescription, v.analysis_preset, v.groups, v.motion)
      .workspace?.motion?.study,
  ).toEqual(m.study);
});
it("builds focus from selected configuration and zoom from all configurations", () => {
  const m = load().workspace!.motion!,
    s = defaultStudy(m);
  expect(studyOptions(m, s, 2).configurations).toHaveLength(4);
  s.mode = "focus";
  expect(studyOptions(m, s, 2).configurations).toEqual([m.configurations[2]]);
  expect(() => studyOptions(m, s, 99)).toThrow();
});
it("invalidates optical provenance on group/configuration edits but not record metadata", () => {
  const m = load().workspace!.motion!,
    key = studyKey(m);
  m.application_revision = 3;
  m.active_id = m.configurations[0].id;
  expect(studyKey(m)).toBe(key);
  m.configurations[0].positions_mm[0] += 0.1;
  expect(studyKey(m)).not.toBe(key);
});
it("rejects corrupt study data instead of silently reopening unsafe settings", () => {
  const p = load(),
    m = p.workspace!.motion!;
  m.study = { settings: defaultStudy(m) };
  m.study.settings.steps = 1000;
  expect(validStudy(m.study)).toBe(false);
  expect(() => projectFromJson(JSON.stringify(p))).toThrow();
  m.study.settings = defaultStudy(m);
  m.study.settings.variables = [
    { kind: "position", configuration: -1, group: 0, min: 0, max: 1 },
  ];
  expect(validStudy(m.study)).toBe(false);
});
it("CSV retains failed rows with blank metrics and quotes diagnostics", () => {
  const r = {
    points: [
      {
        coordinate: 0,
        configuration: {
          name: 'bad,"row',
          object_distance: null,
          positions_mm: [1, 2],
        },
        rms_um: null,
        accepted: false,
        diagnostic: 'blocked,"edge',
      },
    ],
  } as unknown as StudyResult;
  const csv = studyCsv(r);
  expect(csv).toContain('"bad,""row"');
  expect(csv).toContain('"false"');
  expect(csv).toContain('"blocked,""edge"');
  expect(csv.split("\n")).toHaveLength(2);
});
