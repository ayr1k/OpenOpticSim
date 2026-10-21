import { useEffect, useRef, useState } from "react";
import './spatial.css';
import type { Project } from "./types";
import { compute, exportReport } from "./engine";
import { ensureWorkspace, prescription } from "./versions";
import {
  defaultSpatial,
  identityPose,
  topology,
  spatialWireframes,
  type Pose,
} from "./spatial";
interface Ray {
  ray_index: number;
  wavelength_nm: number;
  points: number[][];
  status: string;
  surface: number | null;
  optical_path_mm: number;
  hits: {
    surface: number;
    position_mm: number[];
    local_mm: number[];
    normal: number[];
    incident: number[];
    outgoing: number[] | null;
    n_before: number;
    n_after: number;
    incidence_rad: number;
    refraction_rad: number | null;
    optical_path_mm: number;
  }[];
}
export default function SpatialPanel({
  project,
  edit,
}: {
  project: Project;
  edit: (f: (p: Project) => void) => void;
}) {
  const saved = project.workspace?.spatial ?? defaultSpatial(project),
    options = saved.options;
  const [surface, setSurface] = useState(0),
    [rays, setRays] = useState<Ray[]>([]),
    [selected, setSelected] = useState(0),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [angle, setAngle] = useState(20);
  const controller = useRef<AbortController | null>(null);
  const key = JSON.stringify([prescription(project), saved]);
  useEffect(() => {
    controller.current?.abort();
    setRays([]);
    setBusy(false);
    setError("");
    return () => controller.current?.abort();
  }, [key]);
  const stale = saved.topology !== topology(project);
  const groups = project.workspace?.groups ?? [],
    group = groups[surface - project.surfaces.length - 1];
  const pose = group
    ? (options.groups?.find(
        (g) => g.start === group.start && g.end === group.end,
      )?.pose ?? identityPose())
    : surface === project.surfaces.length
      ? options.detector
      : (options.poses[surface] ?? identityPose());
  function change(property: keyof Pose, axis: number, value: number) {
    if (!Number.isFinite(value)) return;
    edit((p) => {
      const s = (ensureWorkspace(p).spatial ??= defaultSpatial(p));
      let target: Pose;
      if (group) {
        const gs = (s.options.groups ??= []);
        let g = gs.find((g) => g.start === group.start && g.end === group.end);
        if (!g) {
          g = { start: group.start, end: group.end, pose: identityPose() };
          gs.push(g);
        }
        target = g.pose;
      } else
        target =
          surface === p.surfaces.length
            ? s.options.detector
            : (s.options.poses[surface] ??= identityPose());
      target[property][axis] = value;
    });
  }
  async function run() {
    const c = new AbortController();
    controller.current = c;
    setBusy(true);
    setError("");
    try {
      const result = await compute<Ray[]>(
        "spatial",
        "trace_spatial",
        project,
        c.signal,
        { spatial_options: options },
      );
      if (!c.signal.aborted) {
        setRays(result);
        setSelected(0);
      }
    } catch (e) {
      if (!c.signal.aborted) setError(String(e));
    } finally {
      if (!c.signal.aborted) setBusy(false);
    }
  }
  const wires = stale ? [] : spatialWireframes(project, options);
  const all = [...rays.flatMap((r) => r.points), ...wires.flat()],
    a = (angle * Math.PI) / 180;
  const projected = all.map((p) => [
    p[2] * Math.cos(a) + p[0] * Math.sin(a),
    p[1] * Math.cos(0.35) +
      (p[0] * Math.cos(a) - p[2] * Math.sin(a)) * Math.sin(0.35),
  ]);
  const minX = Math.min(0, ...projected.map((p) => p[0])),
    maxX = Math.max(1, ...projected.map((p) => p[0])),
    minY = Math.min(-1, ...projected.map((p) => p[1])),
    maxY = Math.max(1, ...projected.map((p) => p[1]));
  const scale = Math.min(860 / (maxX - minX), 300 / (maxY - minY));
  const point = (p: number[]) =>
    `${20 + (p[2] * Math.cos(a) + p[0] * Math.sin(a) - minX) * scale},${330 - (p[1] * Math.cos(0.35) + (p[0] * Math.cos(a) - p[2] * Math.sin(a)) * Math.sin(0.35) - minY) * scale}`;
  const ray = rays[selected];
  return (
    <section className="panel spatial-panel">
      <h2>三维顺序追迹 · 光线调试器</h2>
      <p>
        这是独立保存的三维实验配置。其他画质、优化和图纸页面仍分析原始同轴处方；偏心、倾斜尚不参与
        PSF / MTF。这里使用显式发射光线，不把未瞄准的光束当作瞳面采样。
      </p>
      {stale && (
        <p role="alert">表面顺序或名称已改变，请重置三维配置后继续。</p>
      )}
      <button
        onClick={() =>
          edit((p) => {
            ensureWorkspace(p).spatial = defaultSpatial(p);
          })
        }
      >
        重置三维配置
      </button>
      <label>
        编辑对象{" "}
        <select
          value={Math.min(surface, project.surfaces.length + groups.length)}
          onChange={(e) => setSurface(+e.target.value)}
        >
          {project.surfaces.map((s, i) => (
            <option key={i} value={i}>
              S{i + 1} · {s.name}
            </option>
          ))}
          <option value={project.surfaces.length}>传感器</option>
          {groups.map((g, i) => (
            <option key={g.id} value={project.surfaces.length + 1 + i}>
              镜组 · {g.name}
            </option>
          ))}
        </select>
      </label>
      <p>
        位移相对名义顶点，转角顺序 Rx → Ry →
        Rz。单面绕自身顶点旋转；镜组绕组首名义顶点整体旋转，再平移。重叠镜组会拒绝计算。
      </p>
      {(["offset_mm", "rotation_rad"] as const).map((group) => (
        <div key={group}>
          {["X", "Y", "Z"].map((axis, i) => (
            <label key={axis}>
              {axis} {group === "offset_mm" ? "mm" : "°"}{" "}
              <input
                type="number"
                step={group === "offset_mm" ? 0.1 : 0.01}
                value={
                  pose[group][i] *
                  (group === "rotation_rad" ? 180 / Math.PI : 1)
                }
                disabled={stale}
                onChange={(e) =>
                  change(
                    group,
                    i,
                    +e.target.value *
                      (group === "rotation_rad" ? Math.PI / 180 : 1),
                  )
                }
              />
            </label>
          ))}
        </div>
      ))}
      <details>
        <summary>发射光线与资源</summary>
        <p>坐标 mm、方向向量、波长 nm。每行一条光线，支持斜光线。</p>
        {options.rays.map((r, i) => (
          <div key={i}>
            光线 {i + 1}{" "}
            {(["origin_mm", "direction"] as const).map((field) =>
              r[field].map((x, j) => (
                <input
                  aria-label={`光线${i + 1} ${field} ${j}`}
                  key={field + j}
                  type="number"
                  step="0.1"
                  value={x}
                  onChange={(e) => {
                    const value = +e.target.value;
                    if (Number.isFinite(value))
                      edit((p) => {
                        const s = (ensureWorkspace(p).spatial ??=
                          defaultSpatial(p));
                        s.options.rays[i][field][j] = value;
                      });
                  }}
                />
              )),
            )}
          </div>
        ))}
        <label>
          最大工作线程{" "}
          <input
            type="number"
            min="1"
            max="64"
            value={options.threads}
            onChange={(e) =>
              edit((p) => {
                (ensureWorkspace(p).spatial ??=
                  defaultSpatial(p)).options.threads = Math.max(
                  1,
                  Math.min(64, Math.round(+e.target.value) || 1),
                );
              })
            }
          />
        </label>
      </details>
      <button
        disabled={busy || stale || !project.surfaces.length}
        onClick={() => void run()}
      >
        追迹并检查
      </button>
      <button
        disabled={!busy}
        onClick={() => {
          controller.current?.abort();
          setBusy(false);
        }}
      >
        取消
      </button>
      <button
        onClick={() =>
          void exportReport("wollaston-request.json", {
            operation: "trace_spatial",
            project: prescription(project),
            spatial_options: options,
          })
        }
      >
        导出可复现请求
      </button>
      {error && <p role="alert">{error}</p>}
      {!!rays.length && (
        <>
          <label>
            三维视角{" "}
            <input
              type="range"
              min="-180"
              max="180"
              value={angle}
              onChange={(e) => setAngle(+e.target.value)}
            />
          </label>
          <svg
            viewBox="0 0 900 360"
            role="img"
            aria-label="三维光线路径，失败路径截至最后实际交点"
            style={{ width: "100%", background: "#0c161d" }}
          >
            {wires.map((line, i) => (
              <polyline
                key={`surface-${i}`}
                points={line.map(point).join(" ")}
                fill="none"
                stroke={i === wires.length - 1 ? "#e7bb7c" : "#5fa9b1"}
                opacity="0.6"
                strokeWidth="0.8"
              />
            ))}
            {rays.map((r, i) => (
              <polyline
                key={i}
                points={r.points.map(point).join(" ")}
                fill="none"
                stroke={r.status === "complete" ? "#b9e38e" : "#ed997b"}
                strokeWidth={i === selected ? 3 : 1}
                onClick={() => setSelected(i)}
              />
            ))}
            <text x="20" y="20" fill="white">
              全局坐标 · 等比例投影 · mm
            </text>
          </svg>
          <label>
            检查光线{" "}
            <select
              value={selected}
              onChange={(e) => setSelected(+e.target.value)}
            >
              {rays.map((r, i) => (
                <option key={i} value={i}>
                  #{i + 1} · {r.status}
                  {r.surface !== null ? ` · S${r.surface + 1}` : ""}
                </option>
              ))}
            </select>
          </label>
          <p>
            总光程 {ray?.optical_path_mm.toFixed(6)} mm · {ray?.wavelength_nm}{" "}
            nm
          </p>
          <div style={{ overflow: "auto", maxHeight: 360 }}>
            <table>
              <thead>
                <tr>
                  <th>表面</th>
                  <th>全局交点 mm</th>
                  <th>法线</th>
                  <th>n → n′</th>
                  <th>入射 / 折射 °</th>
                  <th>累计光程 mm</th>
                </tr>
              </thead>
              <tbody>
                {ray?.hits.map((h) => (
                  <tr key={h.surface}>
                    <td>S{h.surface + 1}</td>
                    <td>{h.position_mm.map((x) => x.toFixed(5)).join(", ")}</td>
                    <td>{h.normal.map((x) => x.toFixed(4)).join(", ")}</td>
                    <td>
                      {h.n_before.toFixed(5)} → {h.n_after.toFixed(5)}
                    </td>
                    <td>
                      {((h.incidence_rad * 180) / Math.PI).toFixed(4)} /{" "}
                      {h.refraction_rad === null
                        ? "—"
                        : ((h.refraction_rad * 180) / Math.PI).toFixed(4)}
                    </td>
                    <td>{h.optical_path_mm.toFixed(6)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <button
            onClick={() => void exportReport("wollaston-rays.json", rays)}
          >
            导出逐面光线记录
          </button>
        </>
      )}
    </section>
  );
}
