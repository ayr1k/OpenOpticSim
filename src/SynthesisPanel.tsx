import { useEffect, useRef, useState } from "react";
import type { Candidate, Project } from "./types";
import { compute, exportReport } from "./engine";
import { ConstraintTable, checkLabel } from "./ConstraintEditor";
import { Num } from "./Num";
import {
  type DesignBrief,
  defaultTask,
  applyTask,
  projectFromCandidate,
} from "./designTask";
import { ensureWorkspace } from "./versions";
export type { DesignBrief } from "./designTask";
interface Outcome {
  candidate: Candidate | null;
  candidates: Candidate[];
  attempts: string[];
  brief: DesignBrief;
  note: string;
}
export default function SynthesisPanel({
  project,
  onCreate,
  edit,
}: {
  project: Project;
  edit: (f: (p: Project) => void) => void;
  onCreate: (p: Project) => void;
}) {
  const brief = project.workspace?.design_task ?? defaultTask;
  const [result, setResult] = useState<Outcome | null>(null),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [progress, setProgress] = useState("");
  const ctrl = useRef<AbortController | null>(null);
  const taskKey = JSON.stringify(brief);
  useEffect(() => {
    ctrl.current?.abort();
    setBusy(false);
    setResult(null);
    return () => ctrl.current?.abort();
  }, [taskKey]);
  const patch = (v: Partial<DesignBrief>) => {
    ctrl.current?.abort();
    setBusy(false);
    setResult(null);
    edit((p) => {
      ensureWorkspace(p).design_task = { ...brief, ...v };
    });
  };
  const run = async () => {
    ctrl.current?.abort();
    const c = new AbortController();
    ctrl.current = c;
    setBusy(true);
    setError("");
    setResult(null);
    setProgress("");
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
    auto: "自动比较全部四种结构",
    tessar: "Tessar 型 · 4 片 3 组",
    planar: "Planar 型 · 6 片 4 组",
    double_gauss: "分离式双高斯 · 6 片 6 组",
    sonnar: "Sonnar 型 · 6 片 3 组",
  };
  return (
    <div className="workflow-panel">
      <h2>设计任务 · 从指标创建设计</h2>
      <p>任务随项目保存，可撤销。先设定目标，再生成新设计或应用到当前镜头。</p>
      <label style={{ display: "block", marginBottom: 8 }}>
        <input
          type="checkbox"
          checked={brief.require_sensor_coverage}
          disabled={busy}
          onChange={(e) => patch({ require_sensor_coverage: e.target.checked })}
        />
        必须覆盖传感器短边、长边与角落（实际像高求解）
      </label>
      <label style={{ display: "block", marginBottom: 8 }}>
        <input
          type="checkbox"
          checked={brief.glass_search}
          disabled={busy}
          onChange={(e) => patch({ glass_search: e.target.checked })}
        />
        自动玻璃选择：比较 3 组目录玻璃，每组重新优化几何
      </label>
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
      <button
        disabled={busy}
        onClick={() =>
          patch({
            ...defaultTask,
            sensor_mm: [6, 4],
            max_length_mm: 150,
            glass_search: true,
          })
        }
      >
        加载已验证入门任务 · 50 mm F8 / 6×4 mm
      </button>
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
        覆盖验收开启时，以实际像高为目标，逐步扩展至传感器检查点；角度设置仅用于关闭覆盖验收的模式。几何通过不代表
        MTF 或制造公差通过。
      </p>
      <p className="hint">
        玻璃池：N-BK7/N-F2、N-BAK4/N-SF5、N-SK16/N-SF10；保留材料来源和系数。每个组合使用相同目标与预算。此版本不穷举全部玻璃。
      </p>
      <button disabled={busy} onClick={() => edit((p) => applyTask(p, brief))}>
        将任务应用到当前镜头的设计目标与优化约束
      </button>
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
          <p>
            同一任务、F/d/C 波长、共用平面像面、33×33
            瞳采样。排序优先满足全部约束，再比较 RMS。
          </p>
          <table>
            <thead>
              <tr>
                <th>候选 / 材料</th>
                <th>RMS µm</th>
                <th>未通过项</th>
                <th>查看</th>
              </tr>
            </thead>
            <tbody>
              {result.candidates.map((c, i) => (
                <tr key={i}>
                  <td>{c.project.name}</td>
                  <td>{c.after_rms_um.toFixed(3)}</td>
                  <td>
                    {c.checks
                      .filter((v) => !v.passed)
                      .map(checkLabel)
                      .join("、") || "全部通过"}
                  </td>
                  <td>
                    <button
                      onClick={() => setResult({ ...result, candidate: c })}
                    >
                      查看检查表
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {result.candidate && (
            <>
              <ConstraintTable checks={result.candidate.checks} />
              <button
                disabled={!result.candidate.accepted}
                onClick={() => {
                  onCreate(projectFromCandidate(result.candidate!, brief));
                }}
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
