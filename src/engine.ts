import { tasks } from "./taskQueue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  Project,
  Variable,
  WaveOptions,
  FocusOptions,
  Constraints,
  OptimizationOptions,
  OptimizationProgress,
  ApertureOptions,
} from "./types";
import { prescription } from "./versions";
import { download, projectFromJson } from "./project";
import repaired400 from "../examples/400mm-coverage-0.9.oos.json";
import wide35 from "../examples/35mm-coverage.oos.json";
import zoom2470 from "../examples/24-70mm-zoom.oos.json";
import tessar from "../examples/tessar-50mm-1.3.oos.json";
import planar from "../examples/planar-50mm-1.3.oos.json";
import gauss from "../examples/double_gauss-50mm-1.3.oos.json";
import sonnar from "../examples/sonnar-50mm-1.3.oos.json";
export const native = "__TAURI_INTERNALS__" in window;
export async function exportPdf(
  name: string,
  bytes: Uint8Array,
): Promise<boolean> {
  if (bytes.length > 32_000_000)
    throw Error("PDF 超过 32 MB，请关闭逐视场衍射图或减少项目历史后重试。");
  if (native)
    return !!(await invoke("save_pdf", { name, bytes: Array.from(bytes) }));
  const url = URL.createObjectURL(
    new Blob([bytes as Uint8Array<ArrayBuffer>], { type: "application/pdf" }),
  );
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 10000);
  return true;
}
export function openNativeProject(): Promise<{
  name: string;
  content: string;
  path: string;
} | null> {
  return invoke("open_project");
}
export function saveNativeProject(
  name: string,
  content: string,
  csv: boolean,
  path: string | null = null,
): Promise<string | null> {
  return invoke("save_project", { name, content, csv, path });
}
export async function exportReport(
  name: string,
  data: unknown,
): Promise<boolean> {
  const text = JSON.stringify(data, null, 2);
  if (new TextEncoder().encode(text).length > 4_000_000)
    throw Error("报告超过 4 MB，请减少视场或光谱数量。");
  if (native) return !!(await saveNativeProject(name, text, false));
  download(name, text);
  return true;
}
export async function examples(): Promise<Project[]> {
  if (native)
    return [
      ...(await invoke<Project[]>("examples")),
      projectFromJson(JSON.stringify(repaired400)),
      projectFromJson(JSON.stringify(wide35)),
      projectFromJson(JSON.stringify(zoom2470)),
      ...[tessar, planar, gauss, sonnar].map((p) =>
        projectFromJson(JSON.stringify(p)),
      ),
    ];
  const r = await fetch("/api/examples", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: "{}",
  });
  const data = await r.json();
  if (!r.ok) throw Error(data.error);
  return [
    ...data,
    projectFromJson(JSON.stringify(repaired400)),
    projectFromJson(JSON.stringify(wide35)),
    projectFromJson(JSON.stringify(zoom2470)),
    ...[tessar, planar, gauss, sonnar].map((p) =>
      projectFromJson(JSON.stringify(p)),
    ),
  ];
}
async function computeDirect<T>(
  job: string,
  operation: string,
  project: Project,
  signal: AbortSignal,
  extras: {
    sampling?: number;
    spatial_options?: import('./spatial').SpatialOptions;
    diffraction?: boolean;
    variables?: Variable[];
    target_efl?: number;
    pattern?: string;
    wave_options?: WaveOptions;
    focus_options?: FocusOptions;
    constraints?: Constraints;
    optimization?: OptimizationOptions;
    aperture_options?: ApertureOptions;
    motion_options?: import("./types").MotionOptions;
    study_options?: import("./types").StudyOptions;
    design_brief?: import("./SynthesisPanel").DesignBrief;
    search_checkpoint?: import("./searchCheckpoint").SearchCheckpoint;
    group_move?: { start: number; end: number; delta_mm: number };
  } = {},
  onProgress?: (p: OptimizationProgress) => void,
): Promise<T> {
  if (signal.aborted) throw new DOMException("Cancelled", "AbortError");
  const request = { operation, project: prescription(project), ...extras };
  if (native) {
    const unlisten = onProgress
      ? await listen<{ job: string; progress: OptimizationProgress }>(
          "optimization-progress",
          (event) => {
            if (event.payload.job === job && !signal.aborted)
              onProgress(event.payload.progress);
          },
        )
      : undefined;
    const abort = () => {
      void invoke("cancel", { job });
    };
    signal.addEventListener("abort", abort, { once: true });
    try {
      if (signal.aborted) throw new DOMException("Cancelled", "AbortError");
      return await invoke("compute", { job, request });
    } finally {
      unlisten?.();
      signal.removeEventListener("abort", abort);
    }
  }
  const r = await fetch(onProgress ? "/api/compute-progress" : "/api/compute", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
    signal,
  });
  if (onProgress && r.ok && r.body) {
    const reader = r.body.getReader(),
      decoder = new TextDecoder();
    let buffer = "",
      result: T | undefined;
    const consume = (line: string) => {
      if (!line.trim()) return;
      const item = JSON.parse(line);
      if (item.error) throw Error(explainStreamError(item.error));
      if (item.progress && !signal.aborted) onProgress(item.progress);
      if ("result" in item) result = item.result;
    };
    try {
      for (;;) {
        const { done, value } = await reader.read();
        buffer += decoder.decode(value, { stream: !done });
        const lines = buffer.split("\n");
        buffer = lines.pop()!;
        lines.forEach(consume);
        if (done) break;
      }
      consume(buffer);
      if (signal.aborted) throw new DOMException("Cancelled", "AbortError");
      if (result === undefined)
        throw Error("优化数据流未返回结果，请重新计算。");
      return result;
    } finally {
      reader.releaseLock();
    }
  }
  const data = await r.json();
  if (!r.ok) throw Error(data.error ?? JSON.stringify(data));
  return data;
}
export async function compute<T>(
  ...args: Parameters<typeof computeDirect<T>>
): Promise<T> {
  const [job, operation, project, signal, extras, onProgress] = args;
  return tasks.run(
    `${project.name} · ${operation}`,
    signal,
    (queuedSignal, detail) =>
      computeDirect<T>(job, operation, project, queuedSignal, extras, (p) => {
        detail(
          `迭代 ${p.iteration}/${p.max_iterations} · RMS ${p.weighted_rms_um.toFixed(3)} µm`,
        );
        onProgress?.(p);
      }),
  );
}
function explainStreamError(error: string) {
  try {
    return JSON.parse(error).error ?? error;
  } catch {
    return error;
  }
}
