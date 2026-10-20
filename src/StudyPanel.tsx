import { useEffect, useRef, useState } from "react";
import type {
  Project,
  StudySettings,
  StudyResult,
  MotionResult,
  StudyPoint,
  StudyVariable,
} from "./types";
import { compute, exportReport, native, saveNativeProject } from "./engine";
import { download } from "./project";
import {
  defaultStudy,
  studyKey,
  studyOptions,
  studyModes,
  studyCsv,
} from "./study";
import { applyMotion, motionOptions } from "./motion";
import { Num } from "./Num";
import { Layout, Spots } from "./Plots";
import Coverage from "./Coverage";
import { explainError } from "./errors";
import { spotExtent } from "./versions";

const fmt = (v: number | null | undefined, n = 3) =>
  v == null ? "—" : v.toFixed(n);
type Metric =
  | "rms_um"
  | "efl_mm"
  | "f_number"
  | "distortion_percent"
  | "geometric_fraction"
  | "group_position"
  | "min_air_mm";
const metrics: Record<Metric, string> = {
  rms_um: "RMS / μm",
  efl_mm: "实际焦距 / mm",
  f_number: "F 数",
  distortion_percent: "最大绝对畸变 / %",
  geometric_fraction: "几何通光比例",
  min_air_mm: "最小空气间隙采样估计 / mm",
  group_position: "镜组运动位移 / mm",
};
function Chart({
  points,
  metric,
  select,
  selected,
  group,
}: {
  points: StudyPoint[];
  metric: Metric;
  select: (i: number) => void;
  selected: number;
  group: number;
}) {
  const value = (p: StudyPoint | undefined) =>
    !p
      ? null
      : metric === "group_position"
        ? (p.configuration.positions_mm[group] ?? null)
        : p[metric];
  const values = points.map(value).filter((v): v is number => v !== null),
    lo = Math.min(0, ...values),
    hi = Math.max(lo + 1, ...values);
  const x = (i: number) => 45 + (680 * i) / Math.max(1, points.length - 1),
    y = (v: number) => 170 - (135 * (v - lo)) / (hi - lo);
  return (
    <svg
      viewBox="0 0 760 205"
      role="img"
      aria-label={`${metrics[metric]}，按扫描顺序排列，点击位置查看`}
    >
      <text x="5" y="20" fill="currentColor" fontSize="11">
        {fmt(hi)}
      </text>
      <text x="5" y="174" fill="currentColor" fontSize="11">
        {fmt(lo)}
      </text>
      <line x1="45" x2="725" y1="170" y2="170" stroke="currentColor" />
      {points.map((p, i) => {
        const v = value(p),
          prev = value(points[i - 1]);
        return (
          <g key={i}>
            {i > 0 && v !== null && prev != null && (
              <line
                x1={x(i - 1)}
                y1={y(prev)}
                x2={x(i)}
                y2={y(v)}
                stroke="#9ebdfb"
                strokeDasharray="4 3"
              />
            )}
            <circle
              role="button"
              tabIndex={0}
              aria-label={`检查位置 ${i + 1} ${p.configuration.name}`}
              cx={x(i)}
              cy={v === null ? 180 : y(v)}
              r={i === selected ? 7 : 5}
              fill={p.accepted ? "#b6dc91" : "#ed9292"}
              onClick={() => select(i)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") select(i);
              }}
            >
              <title>
                {p.configuration.name}: {fmt(v)} · {p.diagnostic}
              </title>
            </circle>
          </g>
        );
      })}
      <text x="45" y="200" fill="currentColor" fontSize="11">
        位置 1
      </text>
      <text x="620" y="200" fill="currentColor" fontSize="11">
        位置 {points.length} · 虚线仅连接采样点
      </text>
    </svg>
  );
}
export default function StudyPanel({
  project,
  edit,
  selectedConfiguration,
}: {
  project: Project;
  edit: (f: (p: Project) => void) => void;
  selectedConfiguration: number;
}) {
  const m = project.workspace!.motion!;
  const s = m.study?.settings ?? defaultStudy(m),
    key =
      studyKey(m) +
      (s.mode === "focus" ? `focus:${selectedConfiguration}` : ""),
    settingsKey = JSON.stringify(s);
  const [local, setLocal] = useState<{
    source_key: string;
    settings: StudySettings;
    result: StudyResult;
  } | null>(null);
  const record = local ?? m.study?.saved,
    result = record?.result;
  const stale =
    !!record &&
    (record.source_key !== key ||
      JSON.stringify(record.settings) !== settingsKey);
  const [busy, setBusy] = useState(false),
    [progress, setProgress] = useState(""),
    [error, setError] = useState("");
  const [selected, setSelected] = useState(0),
    [chartGroup, setChartGroup] = useState(0),
    [metric, setMetric] = useState<Metric>("rms_um"),
    [failuresOnly, setFailuresOnly] = useState(false);
  const [detail, setDetail] = useState<MotionResult | null>(null),
    [beforeDetail, setBeforeDetail] = useState<MotionResult | null>(null);
  const [surface, setSurface] = useState(0),
    [variableKind, setVariableKind] =
      useState<StudyVariable["kind"]>("curvature"),
    [variableGroup, setVariableGroup] = useState(0);
  const controller = useRef<AbortController | null>(null);
  useEffect(() => {
    controller.current?.abort();
    setBusy(false);
    setDetail(null);
    setBeforeDetail(null);
    setError("");
    return () => controller.current?.abort();
  }, [key, settingsKey, selectedConfiguration]);
  const update = (f: (v: StudySettings) => void) =>
    edit((p) => {
      const w = p.workspace!.motion!;
      w.study ??= { settings: defaultStudy(w) };
      f(w.study.settings);
    });
  async function run() {
    const abort = new AbortController();
    controller.current?.abort();
    controller.current = abort;
    setBusy(true);
    setError("");
    setLocal(null);
    setDetail(null);
    setBeforeDetail(null);
    setProgress("准备计算；传感器固定，失败位置保留在报告中…");
    try {
      const r = await compute<StudyResult>(
        crypto.randomUUID(),
        "motion_study",
        m.reference,
        abort.signal,
        { study_options: studyOptions(m, s, selectedConfiguration) },
        (p) =>
          setProgress(
            `${s.mode === "optimize" ? "优化轮次" : "已计算位置"} ${p.iteration} / ${p.max_iterations}${s.mode === "optimize" ? "" : "（上限）"} · ${p.step_accepted ? "通过当前检查" : "检查中 / 未通过"}`,
          ),
      );
      if (!abort.signal.aborted) {
        setLocal({ source_key: key, settings: structuredClone(s), result: r });
        setSelected(0);
      }
    } catch (e) {
      if (!abort.signal.aborted) setError(explainError(e));
    } finally {
      if (!abort.signal.aborted) setBusy(false);
    }
  }
  async function inspect(index: number, adopt = false) {
    if (!result || stale || !result.points[index] || (adopt && !local)) return;
    const abort = new AbortController();
    controller.current?.abort();
    controller.current = abort;
    setBusy(true);
    setError("");
    setSelected(index);
    setDetail(null);
    setBeforeDetail(null);
    setProgress("重新追迹选定位置；旧记录不会直接覆盖当前设计…");
    try {
      const point = result.points[index],
        w = {
          ...m,
          reference: result.reference,
          sampling: result.verification_sampling,
        };
      const r = await compute<MotionResult>(
        crypto.randomUUID(),
        "motion",
        result.reference,
        abort.signal,
        { motion_options: motionOptions(w, point.configuration) },
      );
      let baseline: MotionResult | null = null;
      if (result.mode === "optimize")
        baseline = await compute<MotionResult>(
          crypto.randomUUID(),
          "motion",
          m.reference,
          abort.signal,
          { motion_options: motionOptions(w, m.configurations[index]) },
        );
      if (abort.signal.aborted) return;
      if (adopt) {
        if (!point.accepted || !r.accepted)
          throw Error("该位置未通过验收，无法采纳");
        if (result.mode === "optimize" && !result.accepted)
          throw Error("联合优化必须全部配置通过后才能采纳");
        edit((p) => {
          const motion = p.workspace!.motion!;
          if (result.mode === "optimize") {
            motion.reference = structuredClone(result.reference);
            motion.configurations = structuredClone(result.configurations);
          }
          let config = motion.configurations.find(
            (c) => c.id === point.configuration.id,
          );
          if (!config) {
            if (motion.configurations.length >= 30)
              throw Error(
                "最多 30 个配置，请先删除不需要的配置；完整轨迹仍可保存为记录",
              );
            config = {
              ...structuredClone(point.configuration),
              id: crypto.randomUUID(),
            };
            motion.configurations.push(config);
          }
          Object.assign(config, structuredClone(point.configuration), {
            id: config.id,
          });
          config.positions_mm = [...r.positions_mm];
          applyMotion(p, r, config.id);
        });
        setLocal(null);
      } else {
        setDetail(r);
        setBeforeDetail(baseline);
      }
    } catch (e) {
      if (!abort.signal.aborted) setError(explainError(e));
    } finally {
      if (!abort.signal.aborted) setBusy(false);
    }
  }
  const point = result?.points[selected];
  function addVariable() {
    update((v) => {
      if (v.variables.length >= 12) return;
      if (variableKind === "position") {
        const g = m.groups[variableGroup],
          value =
            m.configurations[selectedConfiguration]?.positions_mm[
              variableGroup
            ];
        if (!g || value === undefined) return;
        v.variables.push({
          kind: "position",
          configuration: selectedConfiguration,
          group: variableGroup,
          min: g.min_mm,
          max: g.max_mm,
        });
      } else {
        const value = m.reference.surfaces[surface]?.[variableKind];
        if (value === undefined) return;
        const delta =
          variableKind === "curvature"
            ? Math.max(Math.abs(value) * 0.05, 0.0001)
            : Math.max(value * 0.05, 0.05);
        v.variables.push({
          kind: variableKind,
          surface,
          min:
            variableKind === "thickness"
              ? Math.max(0, value - delta)
              : value - delta,
          max: value + delta,
        });
      }
    });
  }
  return (
    <section className="study-panel">
      <h2>变焦与对焦工作台</h2>
      <p>
        共享处方 · 固定传感器 ·
        逐位置诊断。下方“活动编辑配置”决定对焦扫描起点与位置优化变量所属配置。
      </p>
      <div className="workflow-actions">
        <label>
          工作任务
          <select
            value={s.mode}
            onChange={(e) =>
              update((v) => {
                v.mode = e.target.value as StudySettings["mode"];
              })
            }
          >
            {Object.entries(studyModes).map(([k, v]) => (
              <option key={k} value={k}>
                {v}
              </option>
            ))}
          </select>
        </label>
        {s.mode !== "optimize" && (
          <Num
            label={
              s.mode === "focus" ? "有限物距位置数" : "每段位置数（含端点）"
            }
            value={s.steps}
            min={2}
            max={21}
            step={1}
            onChange={(n) =>
              update((v) => {
                v.steps = Math.round(n);
              })
            }
          />
        )}
        <label>
          复核瞳采样
          <select
            value={s.verification_sampling}
            onChange={(e) =>
              update((v) => {
                v.verification_sampling = +e.target.value;
              })
            }
          >
            {[17, 25, 33].map((n) => (
              <option key={n} value={n}>
                {n} × {n}
              </option>
            ))}
          </select>
        </label>
        <Num
          label="玻璃最薄实体下限"
          unit="mm"
          value={s.min_glass_mm}
          min={0}
          onChange={(n) =>
            update((v) => {
              v.min_glass_mm = n;
            })
          }
        />
        <Num
          label="焦距容差（0=仅报告）"
          unit="%"
          value={s.efl_tolerance_percent ?? 0}
          min={0}
          onChange={(n) =>
            update((v) => {
              v.efl_tolerance_percent = n || null;
            })
          }
        />
        <label>
          <input
            type="checkbox"
            checked={s.require_sensor_coverage}
            onChange={(e) =>
              update((v) => {
                v.require_sensor_coverage = e.target.checked;
              })
            }
          />
          要求传感器全部检查点验证通光
        </label>
      </div>
      {(s.mode === "trajectory" || s.mode === "focus") && (
        <div className="workflow-actions">
          {s.mode === "trajectory" && (
            <label>
              变倍组
              <select
                value={s.driver_group}
                onChange={(e) =>
                  update((v) => {
                    v.driver_group = +e.target.value;
                  })
                }
              >
                {m.groups.map((g, i) => (
                  <option key={g.id} value={i}>
                    {g.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label>
            {s.mode === "focus" ? "对焦组" : "补偿组"}
            <select
              value={s.compensator_group}
              onChange={(e) =>
                update((v) => {
                  v.compensator_group = +e.target.value;
                })
              }
            >
              {m.groups.map((g, i) => (
                <option key={g.id} value={i}>
                  {g.name}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}
      {s.mode === "focus" ? (
        <>
          <div className="workflow-actions">
            <Num
              label="近端物距"
              unit="mm"
              value={s.near_mm}
              min={1.01}
              max={s.far_mm}
              onChange={(n) =>
                update((v) => {
                  v.near_mm = n;
                })
              }
            />
            <Num
              label="远端物距"
              unit="mm"
              value={s.far_mm}
              min={s.near_mm}
              onChange={(n) =>
                update((v) => {
                  v.far_mm = n;
                })
              }
            />
            <label>
              <input
                type="checkbox"
                checked={s.include_infinity}
                onChange={(e) =>
                  update((v) => {
                    v.include_infinity = e.target.checked;
                  })
                }
              />
              包含无限远
            </label>
          </div>
          <p className="hint">
            有限物距按对数间隔采样。视场角变化仅在相同目标像高定义下具有可比性，不将物距不同的倍率直接称为呼吸量。
          </p>
        </>
      ) : (
        s.mode !== "optimize" && (
          <div className="workflow-actions">
            <Num
              label="扫描点数上限"
              value={s.max_points}
              min={2}
              max={101}
              step={1}
              onChange={(n) =>
                update((v) => {
                  v.max_points = Math.round(n);
                })
              }
            />
            <label>
              <input
                type="checkbox"
                checked={s.adaptive}
                onChange={(e) =>
                  update((v) => {
                    v.adaptive = e.target.checked;
                  })
                }
              />
              失败 / 快变区段加密（最多三轮）
            </label>
            <Num
              label="相邻 RMS 变化加密阈值"
              unit="μm"
              value={s.refine_rms_um}
              min={0.01}
              onChange={(n) =>
                update((v) => {
                  v.refine_rms_um = n;
                })
              }
            />
          </div>
        )
      )}
      {s.mode === "trajectory" && (
        <p className="hint">
          按已命名配置的顺序驱动变倍组，每点独立搜索补偿组全行程，再高采样复核。其他组必须保持不动；目标焦距插值仅为比较目标。
        </p>
      )}
      {s.mode === "optimize" && (
        <>
          <p>
            有界坐标搜索，联合评价所有配置与视场。默认禁止任何视场退化；共享厚度变化由最后空气间隔补偿，传感器参考坐标保持固定。材料不替换。
          </p>
          <div className="workflow-actions">
            <Num
              label="最大优化轮次"
              value={s.iterations}
              min={1}
              max={30}
              step={1}
              onChange={(n) =>
                update((v) => {
                  v.iterations = Math.round(n);
                })
              }
            />
            <Num
              label="各配置逐视场最大退化"
              value={s.max_degradation_percent}
              unit="%"
              min={0}
              onChange={(n) =>
                update((v) => {
                  v.max_degradation_percent = n;
                })
              }
            />
            <label>
              变量类型
              <select
                value={variableKind}
                onChange={(e) =>
                  setVariableKind(e.target.value as StudyVariable["kind"])
                }
              >
                <option value="curvature">共享曲率</option>
                <option value="thickness">共享厚度 / 空气间隔</option>
                <option value="position">活动编辑配置的镜组位置</option>
              </select>
            </label>
            {variableKind === "position" ? (
              <label>
                位置变量镜组
                <select
                  value={variableGroup}
                  onChange={(e) => setVariableGroup(+e.target.value)}
                >
                  {m.groups.map((g, i) => (
                    <option key={g.id} value={i}>
                      {g.name}
                    </option>
                  ))}
                </select>
              </label>
            ) : (
              <label>
                共享表面
                <select
                  value={surface}
                  onChange={(e) => setSurface(+e.target.value)}
                >
                  {m.reference.surfaces.map((s, i) => (
                    <option
                      key={i}
                      value={i}
                      disabled={
                        variableKind === "thickness" &&
                        i === m.reference.surfaces.length - 1
                      }
                    >
                      S{i + 1} {s.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
            <button disabled={s.variables.length >= 12} onClick={addVariable}>
              添加联合变量
            </button>
          </div>
          <table>
            <thead>
              <tr>
                <th>变量</th>
                <th>下限</th>
                <th>上限</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {s.variables.map((v, i) => (
                <tr key={i}>
                  <td>
                    {v.kind === "position"
                      ? `${m.configurations[v.configuration]?.name} / ${m.groups[v.group]?.name}`
                      : `S${v.surface + 1} ${v.kind === "curvature" ? "曲率 / mm⁻¹" : "厚度 / mm"}`}
                  </td>
                  <td>
                    <Num
                      label={`变量 ${i + 1} 下限`}
                      value={v.min}
                      onChange={(n) =>
                        update((s) => {
                          s.variables[i].min = n;
                        })
                      }
                    />
                  </td>
                  <td>
                    <Num
                      label={`变量 ${i + 1} 上限`}
                      value={v.max}
                      onChange={(n) =>
                        update((s) => {
                          s.variables[i].max = n;
                        })
                      }
                    />
                  </td>
                  <td>
                    <button
                      onClick={() =>
                        update((s) => {
                          s.variables.splice(i, 1);
                        })
                      }
                    >
                      移除
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
      <p className="hint">
        初步求解采样、RMS
        上限和最小空气间隙沿用下方镜组设置；复核采样不得低于初步采样。RMS
        上限对每个视场单独检查。
      </p>
      <div className="workflow-actions">
        <button disabled={busy} onClick={() => void run()}>
          开始{studyModes[s.mode]}
        </button>
        {busy && (
          <>
            <span role="status">{progress}</span>
            <button
              onClick={() => {
                controller.current?.abort();
                setBusy(false);
                setProgress("已取消");
              }}
            >
              取消工作台计算
            </button>
          </>
        )}
      </div>
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      {record && result && (
        <>
          <h3>
            {studyModes[result.mode]} ·{" "}
            {result.points.filter((p) => p.accepted).length}/
            {result.points.length} 个位置通过
          </h3>
          <p>{result.note}</p>
          {stale && (
            <p className="error">
              输入条件已变化，此记录仅供历史查看；请重新计算后查看光路或采纳。
            </p>
          )}
          {!local && (
            <p className="hint">
              已恢复项目中的扫描记录，可查看光路；采纳前请重新运行完整任务以复核全部约束。
            </p>
          )}
          <div className="workflow-actions">
            <label>
              曲线指标
              <select
                value={metric}
                onChange={(e) => setMetric(e.target.value as Metric)}
              >
                {Object.entries(metrics).map(([k, v]) => (
                  <option key={k} value={k}>
                    {v}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <input
                type="checkbox"
                checked={failuresOnly}
                onChange={(e) => setFailuresOnly(e.target.checked)}
              />
              仅显示未通过位置
            </label>
            {metric === "group_position" && (
              <label>
                曲线镜组
                <select
                  value={chartGroup}
                  onChange={(e) => setChartGroup(+e.target.value)}
                >
                  {m.groups.map((g, i) => (
                    <option key={g.id} value={i}>
                      {g.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
            <button
              disabled={busy || stale}
              onClick={() =>
                edit((p) => {
                  const w = p.workspace!.motion!;
                  w.study ??= { settings: structuredClone(s) };
                  w.study.saved = structuredClone(record);
                })
              }
            >
              将完整记录保存到项目
            </button>
            {(result.mode === "trajectory" || result.mode === "focus") && (
              <button
                disabled={
                  busy ||
                  stale ||
                  !local ||
                  !result.accepted ||
                  result.configurations.length > 30
                }
                onClick={() => {
                  edit((p) => {
                    const w = p.workspace!.motion!;
                    w.configurations = structuredClone(result.configurations);
                    w.active_id = null;
                  });
                  setLocal(null);
                }}
              >
                将已验证轨迹转换为命名配置（替换配置，可撤销，最多 30 个）
              </button>
            )}
            <button
              onClick={() =>
                void exportReport("motion-study.json", record).catch((e) =>
                  setError(explainError(e)),
                )
              }
            >
              导出 JSON 报告
            </button>
            <button
              onClick={() => {
                const text = studyCsv(result);
                if (native)
                  void saveNativeProject("motion-study.csv", text, true).catch(
                    (e) => setError(explainError(e)),
                  );
                else download("motion-study.csv", text);
              }}
            >
              导出位置与指标 CSV
            </button>
          </div>
          <Chart
            group={chartGroup}
            points={result.points}
            metric={metric}
            selected={selected}
            select={(i) => {
              setSelected(i);
              if (!stale && !busy) void inspect(i);
            }}
          />
          <div className="study-table-scroll">
            <table>
              <thead>
                <tr>
                  <th>位置 / 配置</th>
                  <th>物距 mm</th>
                  <th>EFL mm</th>
                  <th>F 数</th>
                  <th>RMS μm</th>
                  <th>最差视场 μm</th>
                  <th>通光比例</th>
                  <th>诊断</th>
                </tr>
              </thead>
              <tbody>
                {result.points.map((p, i) =>
                  failuresOnly && p.accepted ? null : (
                    <tr key={i} aria-selected={selected === i}>
                      <td>
                        <button
                          disabled={busy || stale}
                          onClick={() => void inspect(i)}
                        >
                          {i + 1} · {p.configuration.name}
                        </button>
                      </td>
                      <td>
                        {p.configuration.object_distance === null
                          ? "∞"
                          : fmt(p.configuration.object_distance, 0)}
                      </td>
                      <td>{fmt(p.efl_mm)}</td>
                      <td>{fmt(p.f_number)}</td>
                      <td>{fmt(p.rms_um)}</td>
                      <td>
                        {fmt(
                          p.fields_rms_um.some((v) => v === null) ||
                            !p.fields_rms_um.length
                            ? null
                            : Math.max(...(p.fields_rms_um as number[])),
                        )}
                      </td>
                      <td>{fmt(p.geometric_fraction)}</td>
                      <td>
                        {p.accepted ? "通过" : "未通过"} · {p.diagnostic}
                      </td>
                    </tr>
                  ),
                )}
              </tbody>
            </table>
          </div>
          {point && (
            <div className="group-candidate">
              <h3>
                位置 {selected + 1} · {point.configuration.name}
              </h3>
              <p>
                镜组位移：
                {point.configuration.positions_mm
                  .map((v) => fmt(v))
                  .join(" / ")}{" "}
                mm；最小空气间隙采样估计 {fmt(point.min_air_mm)} mm；横向色差{" "}
                {fmt(point.lateral_color_um)} μm。
              </p>
              <p>
                物方半视场：
                {point.field_angles_rad
                  .map((v) => `${fmt((v * 180) / Math.PI)}°`)
                  .join(" / ")}
                。同一目标像高定义下可比较实际视角变化。
              </p>
              <Coverage value={point.coverage} />
              {result.before[selected] && (
                <p>
                  高采样复核，逐视场 RMS（μm）：
                  {point.fields_rms_um
                    .map(
                      (v, i) =>
                        `${fmt(result.before[selected].fields_rms_um[i])} → ${fmt(v)}`,
                    )
                    .join("；")}
                </p>
              )}
              <button
                disabled={
                  busy ||
                  stale ||
                  !local ||
                  !point.accepted ||
                  (result.mode === "optimize" && !result.accepted)
                }
                onClick={() => void inspect(selected, true)}
              >
                {result.mode === "optimize"
                  ? "采纳全部优化配置与共享处方（可撤销）"
                  : "将此已验证位置加入配置并采纳（可撤销）"}
              </button>
              {detail && (
                <>
                  <Layout
                    project={detail.project}
                    result={detail.analysis}
                    selected={-1}
                    onSelect={() => {}}
                  />
                  {beforeDetail && (
                    <>
                      <p>优化前 · 相同物距、固定像面和点列尺度</p>
                      <Spots
                        result={beforeDetail.analysis}
                        extent={spotExtent([
                          detail.analysis,
                          beforeDetail.analysis,
                        ])}
                      />
                      <p>优化后</p>
                    </>
                  )}
                  <Spots
                    result={detail.analysis}
                    extent={
                      beforeDetail
                        ? spotExtent([detail.analysis, beforeDetail.analysis])
                        : undefined
                    }
                  />
                </>
              )}
            </div>
          )}
        </>
      )}
    </section>
  );
}
