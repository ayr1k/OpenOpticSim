import { explainError } from "./errors";
import { useEffect, useRef, useState } from "react";
import type {
  MotionConfiguration,
  MotionResult,
  MotionWorkspace,
  Project,
} from "./types";
import { compute, exportReport } from "./engine";
import { ensureWorkspace, spotExtent } from "./versions";
import {
  applyMotion,
  createMotion,
  interpolate,
  motionOptions,
} from "./motion";
import { Num } from "./Num";
import { Layout, Spots } from "./Plots";
import Coverage from "./Coverage";
import StudyPanel from "./StudyPanel";

function magnification(r: MotionResult) {
  const d = r.project.object_distance;
  const field = r.analysis.fields.find(
    (f) => Math.abs(f.field) > 1e-8 && f.centroid,
  );
  return d && field?.centroid
    ? Math.abs(field.centroid[1] / (d * Math.tan(field.field)))
    : null;
}
function MovementMap({ m, c }: { m: MotionWorkspace; c: MotionConfiguration }) {
  const vertices: number[] = [];
  let z = 0;
  m.reference.surfaces.forEach((s) => {
    vertices.push(z);
    z += s.thickness;
  });
  const low = Math.min(0, ...m.groups.map((g) => vertices[g.start] + g.min_mm));
  const high = Math.max(z, ...m.groups.map((g) => vertices[g.end] + g.max_mm));
  const x = (v: number) => 110 + ((v - low) * 460) / Math.max(1, high - low);
  return (
    <svg
      viewBox={`0 0 600 ${50 + m.groups.length * 35}`}
      role="img"
      aria-label="固定传感器参考坐标与镜组行程"
    >
      <line
        x1={x(z)}
        y1="10"
        x2={x(z)}
        y2={30 + m.groups.length * 35}
        stroke="#dfb985"
      />
      <text x={x(z) - 45} y="12" fill="currentColor" fontSize="10">
        传感器固定
      </text>
      {m.groups.map((g, i) => {
        const y = 35 + i * 35,
          base = vertices[g.start],
          position = base + c.positions_mm[i];
        return (
          <g key={g.id}>
            <text x="0" y={y + 4} fill="currentColor" fontSize="10">
              {g.name.slice(0, 10)}
            </text>
            <line
              x1={x(base + g.min_mm)}
              x2={x(base + g.max_mm)}
              y1={y}
              y2={y}
              stroke="#7d8b97"
              strokeWidth="5"
            />
            <line
              x1={x(base)}
              x2={x(position)}
              y1={y - 8}
              y2={y - 8}
              stroke="#b6dc91"
            />
            <text x={x(position)} y={y - 10} fill="currentColor" fontSize="10">
              {c.positions_mm[i] >= 0 ? "→" : "←"}{" "}
              {c.positions_mm[i].toFixed(2)}
            </text>
            <rect
              x={x(position)}
              y={y - 5}
              width={Math.max(
                3,
                x(vertices[g.end] + c.positions_mm[i]) - x(position),
              )}
              height="10"
              fill={
                c.positions_mm[i] < g.min_mm || c.positions_mm[i] > g.max_mm
                  ? "#e89999"
                  : "#9ebdfb"
              }
            />
          </g>
        );
      })}
    </svg>
  );
}
export default function MotionPanel({
  project,
  edit,
}: {
  project: Project;
  edit: (f: (p: Project) => void) => void;
}) {
  const m = project.workspace?.motion;
  const [selected, setSelected] = useState(0),
    [solve, setSolve] = useState(0);
  const [result, setResult] = useState<MotionResult | null>(null),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const [comparison, setComparison] = useState<
    { name: string; result: MotionResult | null; error?: string }[]
  >([]);
  const [evaluated, setEvaluated] = useState<MotionConfiguration | null>(null);
  const [zoom, setZoom] = useState(0),
    [progress, setProgress] = useState("");
  const controller = useRef<AbortController | null>(null);
  const key = JSON.stringify(project),
    c = m?.configurations[selected];
  useEffect(() => {
    controller.current?.abort();
    setBusy(false);
    setResult(null);
    setComparison([]);
    setError("");
    return () => controller.current?.abort();
  }, [key, selected]);
  function update(f: (w: MotionWorkspace) => void) {
    edit((p) => {
      const model = ensureWorkspace(p).motion!;
      model.active_id = null;
      f(model);
    });
  }
  function updateConfig(f: (v: MotionConfiguration) => void) {
    update((w) => f(w.configurations[selected]));
  }
  async function run(solver: boolean, configuration = c) {
    if (!m || !configuration) return;
    controller.current?.abort();
    const abort = new AbortController();
    controller.current = abort;
    setBusy(true);
    setError("");
    setResult(null);
    setComparison([]);
    setProgress(solver ? "扫描全行程并局部细化…" : "检查机械间隙并重新追迹…");
    try {
      const r = await compute<MotionResult>(
        crypto.randomUUID(),
        "motion",
        m.reference,
        abort.signal,
        {
          motion_options: motionOptions(
            m,
            configuration,
            solver ? solve : null,
          ),
        },
      );
      if (!abort.signal.aborted) {
        setResult(r);
        setEvaluated(structuredClone(configuration));
      }
    } catch (e) {
      if (!abort.signal.aborted) setError(explainError(e));
    } finally {
      if (!abort.signal.aborted) setBusy(false);
    }
  }
  async function compare(intermediate = false) {
    if (!m) return;
    controller.current?.abort();
    const abort = new AbortController();
    controller.current = abort;
    setBusy(true);
    setResult(null);
    setComparison([]);
    setError("");
    const rows: typeof comparison = [];
    try {
      const configurations =
        intermediate && c && m.configurations[selected + 1]
          ? Array.from({ length: 11 }, (_, i) =>
              interpolate(c, m.configurations[selected + 1], i / 10),
            )
          : m.configurations;
      for (const [i, v] of configurations.entries()) {
        setProgress(`验证 ${i + 1}/${configurations.length}：${v.name}`);
        try {
          rows.push({
            name: v.name,
            result: await compute<MotionResult>(
              crypto.randomUUID(),
              "motion",
              m.reference,
              abort.signal,
              { motion_options: motionOptions(m, v) },
            ),
          });
        } catch (e) {
          if (abort.signal.aborted) return;
          rows.push({ name: v.name, result: null, error: explainError(e) });
        }
        if (abort.signal.aborted) return;
        setComparison([...rows]);
      }
    } catch (e) {
      if (!abort.signal.aborted) setError(explainError(e));
    } finally {
      if (!abort.signal.aborted) setBusy(false);
    }
  }
  if (!m)
    return (
      <div className="workflow-panel">
        <h2>配置与镜组运动</h2>
        <p>
          固定传感器，移动完整镜组实现对焦或变焦。先到“设计 →
          镜组管理”创建镜组（允许单枚镜片），再建立共享参考处方。
        </p>
        <button
          onClick={() => {
            try {
              const next = createMotion(project);
              edit((p) => {
                ensureWorkspace(p).motion = next;
              });
            } catch (e) {
              setError(explainError(e));
            }
          }}
        >
          建立多配置模型
        </button>
        {error && <p className="error">{error}</p>}
      </div>
    );
  const format = (v: number | null | undefined) =>
    v == null ? "未定义" : v.toFixed(3);
  const extent = spotExtent(
    comparison.flatMap((row) => (row.result ? [row.result.analysis] : [])),
  );
  return (
    <div className="workflow-panel motion-panel">
      <StudyPanel
        project={project}
        edit={edit}
        selectedConfiguration={selected}
      />
      <hr />
      <h2>配置与镜组运动 · 固定传感器</h2>
      <p>
        主画布配置：
        {m.configurations.find((v) => v.id === m.active_id)?.name ??
          "未标记 / 编辑后待重新采纳"}
        ；下方编辑配置在检查并采纳后才更新主画布。
      </p>
      <p>
        位移相对共享参考处方，正值向像方。物距从参考首面计量；首组移动时自动转换坐标。现有“对焦扫描”仍用于移动像面。
      </p>
      <div className="workflow-actions">
        <label>
          活动编辑配置
          <select
            value={selected}
            onChange={(e) => setSelected(+e.target.value)}
          >
            {m.configurations.map((v, i) => (
              <option key={v.id} value={i}>
                {v.name}
              </option>
            ))}
          </select>
        </label>
        <button
          disabled={m.configurations.length >= 30 || !c}
          onClick={() => {
            update((w) => {
              w.configurations.push({
                ...structuredClone(c!),
                id: crypto.randomUUID(),
                name: `${c!.name} 副本`.slice(0, 120),
              });
            });
            setSelected(m.configurations.length);
          }}
        >
          复制配置
        </button>
        <button
          disabled={m.configurations.length <= 1 || !c}
          onClick={() => {
            update((w) => {
              const removed = w.configurations.splice(selected, 1)[0];
              if (w.active_id === removed.id) w.active_id = null;
            });
            setSelected(0);
          }}
        >
          删除配置
        </button>
        <button
          disabled={busy}
          onClick={() => {
            try {
              const next = createMotion(project);
              edit((p) => {
                ensureWorkspace(p).motion = next;
              });
              setSelected(0);
            } catch (e) {
              setError(explainError(e));
            }
          }}
        >
          以当前处方重建配置（可撤销）
        </button>
        <button disabled={busy} onClick={() => void compare()}>
          固定像面对比全部配置
        </button>
      </div>
      {c && (
        <>
          <div className="workflow-actions">
            <label>
              配置名称
              <input
                value={c.name}
                maxLength={120}
                onChange={(e) =>
                  updateConfig((v) => {
                    v.name = e.target.value || "配置";
                  })
                }
              />
            </label>
            <label>
              <input
                type="checkbox"
                checked={c.object_distance === null}
                onChange={(e) =>
                  updateConfig((v) => {
                    v.object_distance = e.target.checked ? null : 1000;
                  })
                }
              />
              无限远物体
            </label>
            {c.object_distance !== null && (
              <Num
                label="参考物距"
                value={c.object_distance}
                min={1.01}
                unit="mm"
                onChange={(v) =>
                  updateConfig((c) => {
                    c.object_distance = v;
                  })
                }
              />
            )}
            <Num
              label="目标焦距（仅比较）"
              value={c.target_efl_mm ?? 0}
              min={0}
              unit="mm；0=不设"
              onChange={(v) =>
                updateConfig((c) => {
                  c.target_efl_mm = v || null;
                })
              }
            />
            <label>
              光圈规则
              <select
                value={c.target_f_number === null ? "fixed" : "target"}
                onChange={(e) =>
                  updateConfig((c) => {
                    c.target_f_number = e.target.value === "fixed" ? null : 8;
                  })
                }
              >
                <option value="fixed">保持参考实体光阑</option>
                <option value="target">调整到目标 F 数</option>
              </select>
            </label>
            {c.target_f_number !== null && (
              <Num
                label="目标 F 数"
                value={c.target_f_number}
                min={0.5}
                max={128}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.target_f_number = v;
                  })
                }
              />
            )}
          </div>
          <MovementMap m={m} c={c} />
          <table>
            <thead>
              <tr>
                <th>镜组 / 面号</th>
                <th>行程下限 mm</th>
                <th>行程上限 mm</th>
                <th>位移 mm</th>
                <th>锁定参考位置</th>
              </tr>
            </thead>
            <tbody>
              {m.groups.map((g, i) => (
                <tr key={g.id}>
                  <td>
                    {g.name} S{g.start + 1}–S{g.end + 1}
                  </td>
                  <td>
                    <Num
                      label={`${g.name}下限`}
                      value={g.min_mm}
                      min={-10000}
                      max={g.max_mm}
                      onChange={(v) =>
                        update((w) => {
                          w.groups[i].min_mm = v;
                        })
                      }
                    />
                  </td>
                  <td>
                    <Num
                      label={`${g.name}上限`}
                      value={g.max_mm}
                      min={g.min_mm}
                      max={10000}
                      onChange={(v) =>
                        update((w) => {
                          w.groups[i].max_mm = v;
                        })
                      }
                    />
                  </td>
                  <td>
                    <Num
                      label={`${g.name}位移`}
                      value={c.positions_mm[i]}
                      min={g.min_mm}
                      max={g.max_mm}
                      onChange={(v) =>
                        updateConfig((c) => {
                          c.positions_mm[i] = v;
                        })
                      }
                    />
                    <input
                      aria-label={`${g.name}移动滑杆`}
                      type="range"
                      min={g.min_mm}
                      max={g.max_mm}
                      step="0.001"
                      disabled={g.locked}
                      value={c.positions_mm[i]}
                      onChange={(e) =>
                        updateConfig((c) => {
                          c.positions_mm[i] = +e.target.value;
                        })
                      }
                    />
                  </td>
                  <td>
                    <input
                      aria-label={`${g.name}锁定`}
                      type="checkbox"
                      checked={g.locked}
                      onChange={(e) =>
                        update((w) => {
                          w.groups[i].locked = e.target.checked;
                        })
                      }
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="hint">
            锁定组只接受 0
            位移。缩小行程不会悄悄改写已有配置，越界配置会在计算时给出诊断。
          </p>
          <div className="workflow-actions">
            <Num
              label="最小空气间隙"
              value={m.min_air_mm}
              min={0}
              unit="mm"
              onChange={(v) =>
                update((w) => {
                  w.min_air_mm = v;
                })
              }
            />
            <Num
              label="RMS 验收上限"
              value={m.max_rms_um}
              min={0.001}
              unit="μm"
              onChange={(v) =>
                update((w) => {
                  w.max_rms_um = v;
                })
              }
            />
            <label>
              评价目标
              <select
                value={m.criterion}
                onChange={(e) =>
                  update((w) => {
                    w.criterion = e.target
                      .value as MotionWorkspace["criterion"];
                  })
                }
              >
                <option value="balanced">全部视场等权 RMS</option>
                <option value="center">中心优先（仍检查全部通光）</option>
              </select>
            </label>
            <label>
              采样
              <select
                value={m.sampling}
                onChange={(e) =>
                  update((w) => {
                    w.sampling = +e.target.value;
                  })
                }
              >
                {[7, 17, 25, 33].map((n) => (
                  <option key={n} value={n}>
                    {n} × {n}
                  </option>
                ))}
              </select>
            </label>
            <label>
              对焦 / 补偿组
              <select value={solve} onChange={(e) => setSolve(+e.target.value)}>
                {m.groups.map((g, i) => (
                  <option key={g.id} value={i}>
                    {g.name}
                  </option>
                ))}
              </select>
            </label>
            <button disabled={busy} onClick={() => void run(false)}>
              检查当前配置
            </button>
            <button
              disabled={busy || m.groups[solve]?.locked}
              onClick={() => void run(true)}
            >
              求解对焦 / 变焦补偿
            </button>
          </div>
          <details>
            <summary>手动变焦轨迹与中间位置</summary>
            <p>
              在当前配置与下一配置之间线性插值镜组位置。插值焦距只作为目标；每个位置都重新追迹，不自动保证齐焦。
            </p>
            <input
              aria-label="变焦位置"
              type="range"
              min={0}
              max={1}
              step={0.01}
              value={zoom}
              onChange={(e) => {
                controller.current?.abort();
                setBusy(false);
                setResult(null);
                setZoom(+e.target.value);
              }}
            />
            <span>{Math.round(zoom * 100)}%</span>
            <button
              disabled={busy || !m.configurations[selected + 1]}
              onClick={() => {
                try {
                  void run(
                    false,
                    interpolate(c, m.configurations[selected + 1], zoom),
                  );
                } catch (e) {
                  setError(explainError(e));
                }
              }}
            >
              检查此插值位置
            </button>
            <button
              disabled={busy || !m.configurations[selected + 1]}
              onClick={() => void compare(true)}
            >
              验证 11 个中间位置
            </button>
            <svg
              viewBox="0 0 600 180"
              role="img"
              aria-label="各配置镜组位移曲线"
            >
              <line x1="30" y1="150" x2="570" y2="150" stroke="currentColor" />
              {m.groups.map((g, k) => {
                const low = Math.min(...m.groups.map((g) => g.min_mm)),
                  high = Math.max(...m.groups.map((g) => g.max_mm));
                return (
                  <polyline
                    key={g.id}
                    fill="none"
                    stroke={["#b6dc91", "#9ebdfb", "#dfb985", "#df9bc8"][k % 4]}
                    points={m.configurations
                      .map(
                        (v, i) =>
                          `${30 + (i * 540) / Math.max(1, m.configurations.length - 1)},${150 - ((v.positions_mm[k] - low) * 130) / Math.max(1, high - low)}`,
                      )
                      .join(" ")}
                  />
                );
              })}
            </svg>
            <p>
              {m.groups
                .map(
                  (g, i) => `${i + 1}: ${g.name} [${g.min_mm}, ${g.max_mm}] mm`,
                )
                .join("；")}
            </p>
          </details>
        </>
      )}
      {busy && (
        <p role="status">
          {progress}{" "}
          <button
            onClick={() => {
              controller.current?.abort();
              setBusy(false);
            }}
          >
            取消
          </button>
        </p>
      )}
      {error && <p className="error">{error}</p>}
      {result && (
        <div className="group-candidate">
          <h3>{result.accepted ? "配置通过声明验收" : "配置仍需调整"}</h3>
          <p>{result.diagnostic}</p>
          <p>
            实际焦距 {format(result.analysis.paraxial.efl)} mm · F/
            {format(result.analysis.paraxial.f_number)} · RMS{" "}
            {format(result.score_um)} μm · 有限物距倍率{" "}
            {format(magnification(result))} · 固定传感器参考坐标{" "}
            {format(result.sensor_reference_mm)} mm
          </p>
          {evaluated?.target_efl_mm && (
            <p>
              计算配置目标 {evaluated.target_efl_mm} mm；焦距偏差{" "}
              {format(
                (result.analysis.paraxial.efl ?? NaN) - evaluated.target_efl_mm,
              )}{" "}
              mm
            </p>
          )}
          <p>
            镜组位移：{result.positions_mm.map(format).join(" / ")} mm；原点移动{" "}
            {format(result.origin_shift_mm)} mm。
          </p>
          <Layout
            project={result.project}
            result={result.analysis}
            selected={-1}
            onSelect={() => {}}
          />
          <Coverage value={result.analysis.coverage} />
          <button
            disabled={
              !result.accepted ||
              !evaluated ||
              (evaluated.id === "interpolated" && m.configurations.length >= 30)
            }
            onClick={() =>
              edit((p) => {
                const w = p.workspace!.motion!;
                let config = w.configurations.find(
                  (v) => v.id === evaluated?.id,
                );
                if (!config) {
                  config = {
                    ...structuredClone(evaluated!),
                    id: crypto.randomUUID(),
                  };
                  w.configurations.push(config);
                }
                config.positions_mm = [...result.positions_mm];
                applyMotion(p, result, config.id);
              })
            }
          >
            采纳到当前配置与画布（可撤销）
          </button>
          <button
            onClick={() =>
              void exportReport("motion-report.json", {
                reference: m.reference,
                options: m,
                configuration: evaluated,
                result,
                condition: "固定传感器；未重新移动像面；几何采样；MTF 未验证",
              })
            }
          >
            导出计算报告
          </button>
        </div>
      )}
      {!!comparison.length && (
        <>
          <h3>固定传感器 · 共享视场定义 / 波长 / 采样 · 同尺度点列图</h3>
          <table>
            <thead>
              <tr>
                <th>配置</th>
                <th>EFL mm</th>
                <th>F 数</th>
                <th>RMS μm</th>
                <th>有限物距倍率</th>
                <th>验收</th>
              </tr>
            </thead>
            <tbody>
              {comparison.map((row, i) => (
                <tr key={i}>
                  <td>{row.name}</td>
                  <td>{format(row.result?.analysis.paraxial.efl)}</td>
                  <td>{format(row.result?.analysis.paraxial.f_number)}</td>
                  <td>{format(row.result?.score_um)}</td>
                  <td>
                    {format(row.result ? magnification(row.result) : null)}
                  </td>
                  <td>{row.error ?? row.result?.diagnostic}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {comparison.map(
            (row, i) =>
              row.result && (
                <details key={i}>
                  <summary>{row.name} · 通光与同尺度光斑</summary>
                  <Coverage value={row.result.analysis.coverage} />
                  <Spots result={row.result.analysis} extent={extent} />
                </details>
              ),
          )}
          <button
            onClick={() =>
              void exportReport("motion-comparison.json", {
                reference: m.reference,
                settings: m,
                comparison,
                condition:
                  "固定传感器，各配置物距按记录；未重新对焦；相同点列尺度；未验证外场 MTF",
              })
            }
          >
            导出配置比较报告
          </button>
        </>
      )}
      <p className="hint">
        几何通光率不代表精确相对照度。PSF/MTF
        请在采纳配置后到画质页面单独计算并检查适用范围。
      </p>
    </div>
  );
}
