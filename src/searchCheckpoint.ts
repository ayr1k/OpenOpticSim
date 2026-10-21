import type { Candidate } from "./types";
import { type DesignBrief, validTask } from "./designTask";
export interface SearchCheckpoint {
  engine_version: string;
  brief: DesignBrief;
  cursor: number;
  total: number;
  candidates: Candidate[];
  attempts: string[];
}
export function validCheckpoint(c: SearchCheckpoint): boolean {
  return (
    !!c &&
    c.engine_version === "1.5.0-search1" &&
    validTask(c.brief) &&
    Number.isInteger(c.cursor) &&
    Number.isInteger(c.total) &&
    c.total >= 1 &&
    c.total <= 240 &&
    c.cursor >= 0 &&
    c.cursor <= c.total &&
    Array.isArray(c.attempts) &&
    c.attempts.length === c.cursor &&
    c.attempts.every((s) => typeof s === "string" && s.length < 20000) &&
    Array.isArray(c.candidates) &&
    c.candidates.length <= 8 &&
    c.candidates.every(
      (v) =>
        !!v &&
        Number.isFinite(v.after_rms_um) &&
        v.after_rms_um >= 0 &&
        Array.isArray(v.checks),
    )
  );
}
