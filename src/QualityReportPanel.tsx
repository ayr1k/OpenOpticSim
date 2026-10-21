import { useEffect, useRef, useState } from "react";
import { compute, exportReport } from "./engine";
import { opticalKey } from "./design";
import { prescription } from "./versions";
import type { Analysis, Project, Wave, WaveOptions } from "./types";
import Coverage from "./Coverage";
import { mtfAt } from "./qualityReport";
type Row = {
  angle: number;
  height: number | null;
  rms: number | null;
  delta: number | null;
  converged: boolean;
  mtf: (number | null)[];
  wave_quality: Wave["quality"] | null;
  error: string | null;
};
type Report = {
  source: Project;
  method: string;
  condition: string;
  coverage: Analysis["coverage"];
  rows: Row[];
};
export default function QualityReportPanel({ project }: { project: Project }) {
  const [method, setMethod] = useState<WaveOptions["method"]>("fast"),
    [withMtf, setWithMtf] = useState(false);
  const [report, setReport] = useState<Report | null>(null),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const ctrl = useRef<AbortController | null>(null),
    key = opticalKey(project);
  useEffect(() => {
    ctrl.current?.abort();
    setReport(null);
    setBusy(false);
    return () => ctrl.current?.abort();
  }, [key, method, withMtf]);
  async function run() {
    ctrl.current?.abort();
    const c = new AbortController();
    ctrl.current = c;
    setBusy(true);
    setError("");
    setReport(null);
    try {
      const low = await compute<Analysis>(
        crypto.randomUUID(),
        "analyze",
        project,
        c.signal,
        { sampling: 17 },
      );
      const high = await compute<Analysis>(
        crypto.randomUUID(),
        "analyze",
        project,
        c.signal,
        { sampling: 33 },
      );
      const rows: Row[] = [];
      for (let i = 0; i < high.fields.length; i++) {
        if (c.signal.aborted) break;
        const f = high.fields[i],
          rms = f.polychromatic_rms_um,
          previous = low.fields[i]?.polychromatic_rms_um;
        const delta =
          rms != null && previous != null ? Math.abs(rms - previous) : null;
        const row: Row = {
          angle: f.field,
          height: high.coverage?.fields[i]?.chief_height_mm ?? null,
          rms,
          delta,
          converged:
            delta !== null &&
            rms !== null &&
            delta <= Math.max(0.01, 0.01 * rms) &&
            high.spots
              .filter((s) => s.field === f.field)
              .every((s) => s.failed === 0),
          mtf: [],
          wave_quality: null,
          error: null,
        };
        if (withMtf) {
          try {
            const wave = await compute<Wave>(
              crypto.randomUUID(),
              "wave",
              project,
              c.signal,
              {
                wave_options: {
                  method,
                  field_index: i,
                  wavelength_nm: null,
                  max_pupil_samples: 256,
                },
              },
            );
            row.wave_quality = wave.quality;
            const coverageField = high.coverage?.fields[i];
            if (coverageField) {
              coverageField[method === "sphere_kirchhoff" ? "wide_mtf" : "fast_mtf"] =
                wave.quality.converged ? "converged" : "failed";
            }
            row.mtf = [10, 30, 50].flatMap((freq) =>
              [wave.mtf_sagittal, wave.mtf_tangential].map((values) =>
                wave.quality.converged
                  ? mtfAt(values, wave.frequency_step_lp_mm, freq)
                  : null,
              ),
            );
            if (!wave.quality.converged)
              row.error = "衍射采样未收敛，MTF 不作为有效结果";
          } catch (e) {
            if (c.signal.aborted) throw e;
            row.error = String(e);
            const coverageField = high.coverage?.fields[i];
            const key = method === "sphere_kirchhoff" ? "wide_mtf" : "fast_mtf";
            if (coverageField && coverageField[key] !== "outside_model") coverageField[key] = "failed";
          }
        }
        rows.push(row);
      }
      if (!c.signal.aborted)
        setReport({
          source: prescription(project),
          method: withMtf ? method! : "not_calculated",
          condition:
            "固定当前像面；全视场共用平面传感器；原项目波长与权重；几何采样 17→33；收敛门槛 max(0.01 µm, 1% RMS)。MTF 采用各模型自身收敛门槛。",
          coverage: high.coverage,
          rows,
        });
    } catch (e) {
      if (!c.signal.aborted) setError(String(e));
    } finally {
      if (ctrl.current === c) setBusy(false);
    }
  }
  return (
    <div className="workflow-panel">
      <h2>全视场画质验收报告</h2>
      <p>
        逐视场列出实际像高、RMS 收敛与 10 / 30 / 50 lp/mm 的弧矢 / 子午
        MTF。未计算、超出模型或不收敛均保留为未验证。
      </p>
      <label>
        <input
          type="checkbox"
          checked={withMtf}
          disabled={busy}
          onChange={(e) => setWithMtf(e.target.checked)}
        />
        逐视场计算 MTF（较慢）
      </label>
      <select
        disabled={busy}
        value={method}
        onChange={(e) => setMethod(e.target.value as WaveOptions["method"])}
      >
        <option value="fast">快速参考球模型</option>
        <option value="sphere_kirchhoff">离轴平面像面模型</option>
      </select>
      <button disabled={busy} onClick={() => void run()}>
        生成验收报告
      </button>
      {busy && <button onClick={() => ctrl.current?.abort()}>取消报告</button>}
      {error && <p role="alert">{error}</p>}
      {report && (
        <>
          <p>{report.condition}</p>
          <Coverage value={report.coverage} />
          <table>
            <thead>
              <tr>
                <th>物方角 / 实际像高</th>
                <th>RMS / 变化 µm</th>
                <th>几何收敛</th>
                <th>MTF 10 / 30 / 50（S / T）</th>
              </tr>
            </thead>
            <tbody>
              {report.rows.map((r, i) => (
                <tr key={i}>
                  <td>
                    {((r.angle * 180) / Math.PI).toFixed(3)}° /{" "}
                    {r.height?.toFixed(3) ?? "—"} mm
                  </td>
                  <td>
                    {r.rms?.toFixed(3) ?? "—"} / {r.delta?.toFixed(4) ?? "—"}
                  </td>
                  <td>{r.converged ? "通过" : "未通过"}</td>
                  <td>
                    {r.mtf.length
                      ? r.mtf.map((v) => v?.toFixed(3) ?? "—").join(" / ")
                      : "未计算"}
                    <p>{r.error}</p>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <button
            onClick={() =>
              void exportReport("OOS-quality-acceptance.json", report).catch(
                (e) => setError(String(e)),
              )
            }
          >
            导出可复算报告 JSON
          </button>
        </>
      )}
    </div>
  );
}
