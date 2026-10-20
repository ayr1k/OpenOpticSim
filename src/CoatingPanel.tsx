import { useEffect, useRef, useState } from "react";
import type { Project } from "./types";
import { compute, exportReport } from "./engine";
import { Num } from "./Num";
import type { CoatingLayer } from "./coatings";
interface Report {
  note: string;
  rays: {
    field: number;
    nm: number;
    pupil: number;
    transmission: number | null;
    uncoated?: number | null;
    status: string;
  }[];
}
export default function CoatingPanel({
  project,
  selected,
  edit,
}: {
  project: Project;
  selected: number;
  edit: (f: (p: Project) => void) => void;
}) {
  const [surface, setSurface] = useState(Math.max(0, selected)),
    [result, setResult] = useState<Report | null>(null),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  const ctrl = useRef<AbortController | null>(null);
  const key =
    JSON.stringify(project.surfaces) +
    JSON.stringify([
      project.materials,
      project.fields,
      project.field_spec,
      project.wavelengths,
      project.sensor,
      project.object_distance,
      project.stop,
    ]);
  useEffect(() => {
    ctrl.current?.abort();
    setBusy(false);
    setResult(null);
    setSurface((s) => Math.min(s, Math.max(0, project.surfaces.length - 1)));
    return () => ctrl.current?.abort();
  }, [key]);
  const layers = project.surfaces[surface]?.coating ?? [];
  const update = (next: CoatingLayer[]) =>
    edit((p) => {
      if (next.length) p.surfaces[surface].coating = next;
      else delete p.surfaces[surface].coating;
    });
  return (
    <div className="workflow-panel">
      <h2>镀膜与直透光谱</h2>
      <p>
        层顺序从物方到像方。支持无吸收、均匀、各向同性介质膜；折射率为用户指定常数，不能代替真实膜料的色散数据。
      </p>
      <label>
        表面
        <select
          value={surface}
          onChange={(e) => setSurface(Number(e.target.value))}
        >
          {project.surfaces.map((s, i) => (
            <option key={i} value={i}>
              S{i + 1} · {s.name}
            </option>
          ))}
        </select>
      </label>
      <div className="workflow-actions">
        <button
          disabled={!project.surfaces.length}
          onClick={() =>
            update([
              {
                name: "低折射率示意膜（n=1.38）",
                index: 1.38,
                thickness_nm: 550 / (4 * 1.38),
              },
            ])
          }
        >
          550 nm 四分之一波长单层
        </button>
        <button
          disabled={!project.surfaces.length || layers.length >= 32}
          onClick={() =>
            update([
              ...layers,
              { name: "自定义介质层", index: 1.5, thickness_nm: 100 },
            ])
          }
        >
          添加膜层
        </button>
        <button disabled={!layers.length} onClick={() => update([])}>
          清除此面镀膜
        </button>
      </div>
      {layers.map((l, i) => (
        <div className="dialog-grid" key={i}>
          <label>
            第 {i + 1} 层名称
            <input
              value={l.name}
              onChange={(e) =>
                update(
                  layers.map((a, j) =>
                    j === i ? { ...a, name: e.target.value } : a,
                  ),
                )
              }
            />
          </label>
          <Num
            label="折射率 n"
            value={l.index}
            min={1}
            max={10}
            onChange={(v) =>
              update(layers.map((a, j) => (j === i ? { ...a, index: v } : a)))
            }
          />
          <Num
            label="物理厚度"
            unit="nm"
            value={l.thickness_nm}
            min={0}
            max={10000}
            onChange={(v) =>
              update(
                layers.map((a, j) => (j === i ? { ...a, thickness_nm: v } : a)),
              )
            }
          />
          <button onClick={() => update(layers.filter((_, j) => i !== j))}>
            删除第 {i + 1} 层
          </button>
        </div>
      ))}
      <p className="hint">
        使用项目中的视场和波长，追迹中心及 ±0.8 瞳半径子午光线；分别传递 S/P
        能量。失败光线单独显示，不计为零透射。不改变几何光路，也暂不改变 PSF/MTF
        和测试图。
      </p>
      <button
        disabled={busy || !project.surfaces.length}
        onClick={async () => {
          const c = new AbortController();
          ctrl.current = c;
          setBusy(true);
          setError("");
          try {
            const r = await compute<Report>(
              crypto.randomUUID(),
              "coatings",
              project,
              c.signal,
            );
            if (!c.signal.aborted) setResult(r);
          } catch (e) {
            if (!c.signal.aborted) setError(String(e));
          } finally {
            if (ctrl.current === c) setBusy(false);
          }
        }}
      >
        计算实际光线透射率
      </button>
      {busy && (
        <button
          onClick={() => {
            ctrl.current?.abort();
            setBusy(false);
          }}
        >
          取消
        </button>
      )}
      {error && <p className="error">{error}</p>}
      {result && (
        <>
          <p>{result.note}</p>
          <div className="scroll-table">
            <table>
              <thead>
                <tr>
                  <th>半视场 / 波长</th>
                  <th>瞳位置</th>
                  <th>未镀膜直透</th>
                  <th>当前直透</th>
                  <th>状态</th>
                </tr>
              </thead>
              <tbody>
                {result.rays.map((r, i) => (
                  <tr key={i}>
                    <td>
                      {((r.field * 180) / Math.PI).toFixed(2)}° / {r.nm} nm
                    </td>
                    <td>{r.pupil}</td>
                    <td>
                      {r.uncoated == null
                        ? "—"
                        : (r.uncoated * 100).toFixed(3) + "%"}
                    </td>
                    <td>
                      {r.transmission == null
                        ? "—"
                        : (r.transmission * 100).toFixed(3) + "%"}
                    </td>
                    <td>{r.status}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <button
            onClick={() =>
              void exportReport("OOS-coating-report.json", result).catch((e) =>
                setError(String(e)),
              )
            }
          >
            导出逐面 S/P 与入射角报告
          </button>
        </>
      )}
    </div>
  );
}
