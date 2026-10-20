import { useEffect, useRef, useState } from "react";
import type { Candidate, Project } from "./types";
import { compute, exportReport } from "./engine";
import { ConstraintTable } from "./ConstraintEditor";
import { Num } from "./Num";
export interface DesignBrief {
  structure: string;
  focal_length_mm: number;
  f_number: number;
  sensor_mm: [number, number];
  half_field_deg: number;
  max_length_mm: number;
  max_rms_um: number;
  iterations: number;
  starts: number;
}
interface Outcome {
  candidate: Candidate | null;
  attempts: string[];
  brief: DesignBrief;
  note: string;
}
export default function SynthesisPanel({
  project,
  onCreate,
}: {
  project: Project;
  onCreate: (p: Project) => void;
}) {
  const [brief, setBrief] = useState<DesignBrief>({
    structure: "tessar",
    focal_length_mm: 50,
    f_number: 8,
    sensor_mm: [36, 24],
    half_field_deg: 5,
    max_length_mm: 100,
    max_rms_um: 25,
    iterations: 25,
    starts: 1,
  });
  const [result, setResult] = useState<Outcome | null>(null),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [progress, setProgress] = useState("");
  const ctrl = useRef<AbortController | null>(null);
  useEffect(() => () => ctrl.current?.abort(), []);
  const patch = (v: Partial<DesignBrief>) => {
    ctrl.current?.abort();
    setBusy(false);
    setResult(null);
    setBrief((b) => ({ ...b, ...v }));
  };
  const run = async () => {
    ctrl.current?.abort();
    const c = new AbortController();
    ctrl.current = c;
    setBusy(true);
    setError("");
    setResult(null);
    try {
      const r = await compute<Outcome>(
        crypto.randomUUID(),
        "synthesize",
        project,
        c.signal,
        { design_brief: brief },
        (p) =>
          setProgress(
            `当前阶段 ${p.iteration}/${p.max_iterations} · RMS ${p.weighted_rms_um.toFixed(2)} µm`,
          ),
      );
      if (!c.signal.aborted) setResult(r);
    } catch (e) {
      if (!c.signal.aborted) setError(String(e));
    } finally {
      if (ctrl.current === c) setBusy(false);
    }
  };
  const labels: Record<string, string> = {
    tessar: "Tessar 型 · 4 片 3 组",
    planar: "Planar 型 · 6 片 4 组",
    double_gauss: "分离式双高斯 · 6 片 6 组",
    sonnar: "Sonnar 型 · 6 片 3 组",
  };
  return (
    <div className="workflow-panel">
      <h2>从指标创建设计</h2>
      <p>
        选择结构与目标 → 生成多个初值 → 逐步扩大孔径和视场 →
        全目标复核。当前项目保持打开，确认后创建独立项目。
      </p>
      <label>
        经典结构
        <select
          value={brief.structure}
          disabled={busy}
          onChange={(e) => patch({ structure: e.target.value })}
        >
          {Object.entries(labels).map(([id, label]) => (
            <option key={id} value={id}>
              {label}
            </option>
          ))}
        </select>
      </label>
      <div className="dialog-grid">
        {(
          [
            ["focal_length_mm", "目标焦距", "mm", 10, 1000],
            ["f_number", "目标 F 数", "", 1, 64],
            ["half_field_deg", "最大测试半视场", "°", 0, 40],
            ["max_length_mm", "最大总长", "mm", 1, 10000],
            ["max_rms_um", "各视场 RMS 上限", "µm", 0.1, 10000],
            ["iterations", "每阶段迭代", "", 1, 100],
            ["starts", "确定性初值数量", "", 1, 3],
          ] as const
        ).map(([k, label, unit, min, max]) => (
          <Num
            key={k}
            label={label}
            unit={unit}
            min={min}
            max={max}
            value={brief[k]}
            onChange={(v) =>
              patch({
                [k]: k === "iterations" || k === "starts" ? Math.round(v) : v,
              })
            }
          />
        ))}
        {[0, 1].map((i) => (
          <Num
            key={i}
            label={i ? "传感器高度" : "传感器宽度"}
            unit="mm"
            min={0.1}
            max={100}
            value={brief.sensor_mm[i]}
            onChange={(v) => {
              const sensor_mm = [...brief.sensor_mm] as [number, number];
              sensor_mm[i] = v;
              patch({ sensor_mm });
            }}
          />
        ))}
      </div>
      <p className="hint">
        传感器尺寸不自动扩大测试角度。建议角落半视场约{" "}
        {(
          (Math.atan(
            Math.hypot(...brief.sensor_mm) / 2 / brief.focal_length_mm,
          ) *
            180) /
          Math.PI
        ).toFixed(2)}
        °；存在畸变时须用“设计目标 → 按目标像高”复核。默认 5°
        是窄视场起步，不代表全画幅通过。
      </p>
      <p className="hint">
        原创起点仅借鉴经典拓扑，使用 N-BK7 /
        N-F2。不是原厂镜头复刻；当前不自动选择玻璃、评估公差或保证可制造性。
      </p>
      <button disabled={busy} onClick={() => void run()}>
        生成并优化设计
      </button>
      {busy && (
        <>
          <button
            onClick={() => {
              ctrl.current?.abort();
              setBusy(false);
            }}
          >
            取消
          </button>
          <p role="status">{progress || "正在建立初值…"}</p>
        </>
      )}
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      {result && (
        <>
          <h3>
            {result.candidate?.accepted
              ? "所选几何目标检查通过"
              : "尚未达到设计目标"}
          </h3>
          {result.attempts.map((s, i) => (
            <p key={i}>{s}</p>
          ))}
          <p>{result.note}</p>
          {result.candidate && (
            <>
              <ConstraintTable checks={result.candidate.checks} />
              <button
                disabled={!result.candidate.accepted}
                onClick={() => onCreate(result.candidate!.project)}
              >
                使用已验证候选创建项目
              </button>
            </>
          )}
          <button
            onClick={() =>
              void exportReport("OOS-design-search.json", result).catch((e) =>
                setError(String(e)),
              )
            }
          >
            导出完整搜索报告
          </button>
        </>
      )}
    </div>
  );
}
