import { useEffect, useState } from "react";
import type { Analysis, ConstraintCheck, Project } from "./types";
import { compute } from "./engine";
import { defaultConstraints, ensureWorkspace } from "./versions";
import { defaultOptimization, defaultSetting } from "./optimization";
import { opticalKey } from "./design";
import { ConstraintEditor, ConstraintTable } from "./ConstraintEditor";
import { Num } from "./Num";
import { explainError } from "./errors";
import { validTask, applyTask } from "./designTask";
import FieldPanel from "./FieldPanel";
export default function GoalPanel({
  project,
  result,
  edit,
  optimize,
  disabled,
}: {
  project: Project;
  result: Analysis | null;
  edit: (f: (p: Project) => void) => void;
  optimize: () => void;
  disabled: boolean;
}) {
  const c =
      project.workspace?.constraints ??
      defaultConstraints(project, result?.paraxial.efl ?? 50),
    g = c.design;
  const [checks, setChecks] = useState<ConstraintCheck[]>([]),
    [checkedKey, setCheckedKey] = useState(""),
    [error, setError] = useState("");
  const key = opticalKey(project) + JSON.stringify(c);
  useEffect(() => {
    const ctrl = new AbortController();
    setChecks([]);
    setError("");
    compute<ConstraintCheck[]>(
      crypto.randomUUID(),
      "checks",
      project,
      ctrl.signal,
      { constraints: c },
    )
      .then((v) => {
        if (!ctrl.signal.aborted) {
          setCheckedKey(key);
          setChecks(v);
        }
      })
      .catch((e) => {
        if (!ctrl.signal.aborted) setError(explainError(e));
      });
    return () => ctrl.abort();
  }, [key]);
  const set = (value: typeof c) =>
    edit((p) => {
      const w = ensureWorkspace(p);
      w.constraints = value;
      if (w.design_task) {
        const task = {
          ...w.design_task,
          focal_length_mm: value.target_efl_mm,
          max_length_mm: value.max_length_mm,
          f_number: value.design?.f_number ?? w.design_task.f_number,
        };
        if (validTask(task)) w.design_task = task;
      }
    });
  const actual = result?.paraxial,
    required = g ? c.target_efl_mm / g.f_number : null,
    front = (project.surfaces[0]?.semi_aperture ?? 0) * 2;
  return (
    <div className="workflow-panel design-panel">
      <h2>设计目标与实际能力</h2>
      {project.workspace?.design_task && (
        <p>
          已有统一设计任务：{project.workspace.design_task.focal_length_mm} mm /
          F{project.workspace.design_task.f_number}。
          <button
            onClick={() => edit((p) => applyTask(p, p.workspace!.design_task!))}
          >
            同步任务的传感器、视场与画质验收要求
          </button>{" "}
          修改本页焦距、F 数与长度会同步到任务；手工改变其他约束仍保留。
        </p>
      )}
      <p>
        目标用于约束优化；改变目标不会直接缩放镜头或画面。F 数由 d
        线近轴入瞳计算，通光仍须实际追迹。
      </p>
      <table aria-label="目标与实际对比">
        <thead>
          <tr>
            <th>指标</th>
            <th>目标 / 限制</th>
            <th>实际</th>
            <th>差距</th>
          </tr>
        </thead>
        <tbody>
          {[
            ["有效焦距 mm", c.target_efl_mm, actual?.efl],
            ["F 数", g?.f_number, actual?.f_number],
            ["最小后焦距 mm", g?.min_bfl_mm, actual?.bfl],
            [
              "最大总长 mm",
              c.max_length_mm,
              project.surfaces.reduce((a, s) => a + s.thickness, 0),
            ],
            [
              "最大前组有效直径 mm",
              g?.max_front_diameter_mm,
              project.surfaces.length ? front : undefined,
            ],
          ].map(([name, target, value]) => (
            <tr key={String(name)}>
              <td>{name}</td>
              <td>
                {typeof target === "number" ? target.toFixed(3) : "未启用"}
              </td>
              <td>{typeof value === "number" ? value.toFixed(3) : "—"}</td>
              <td>
                {typeof value === "number" && typeof target === "number"
                  ? (value - target).toFixed(3)
                  : "—"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {required !== null && (
        <p
          className={
            project.object_distance === null &&
            required > Math.min(front, g!.max_front_diameter_mm)
              ? "error"
              : "hint"
          }
        >
          无限远目标所需入瞳直径约 {required.toFixed(2)} mm；当前前组有效直径{" "}
          {front.toFixed(2)} mm。
          {project.object_distance !== null
            ? "有限物距还需要工作 F 数与瞳放大率分析，此处只显示无限远规格估计。"
            : required > Math.min(front, g!.max_front_diameter_mm)
              ? "无限远轴上目标受前口径限制，需要扩大前组或放宽目标；只扩大实体光阑不能解决。"
              : "入瞳需求的尺寸检查通过，仍不代表各视场通光或像差达标。"}
        </p>
      )}
      <div className="design-inputs">
        <div>
          <h3>焦距与尺寸约束</h3>
          <ConstraintEditor value={c} onChange={set} />
        </div>
        <div>
          <h3>摄影规格</h3>
          <label>
            <input
              type="checkbox"
              checked={!!g}
              onChange={(e) =>
                set({
                  ...c,
                  design: e.target.checked
                    ? {
                        f_number: Math.min(
                          128,
                          Math.max(0.5, actual?.f_number ?? 16),
                        ),
                        f_tolerance: 0.1,
                        min_bfl_mm: 1,
                        max_front_diameter_mm: front || 70,
                      }
                    : undefined,
                })
              }
            />
            启用 F 数、后焦距与前组口径约束
          </label>
          {g && (
            <>
              {(
                [
                  ["f_number", "目标 F 数", 0.5, 128],
                  ["f_tolerance", "F 数允许误差", 0.000001, 128],
                  ["min_bfl_mm", "最小后焦距", 0, 1e5],
                  ["max_front_diameter_mm", "最大前组有效直径", 0.001, 2e4],
                ] as const
              ).map(([k, label, min, max]) => (
                <Num
                  key={k}
                  label={label}
                  value={g[k]}
                  min={min}
                  max={max}
                  unit={k.includes("mm") ? "mm" : ""}
                  onChange={(v) => set({ ...c, design: { ...g, [k]: v } })}
                />
              ))}
            </>
          )}
          <Num
            label="目标传感器宽度"
            value={project.sensor[0]}
            min={0.01}
            unit="mm"
            onChange={(v) =>
              edit((p) => {
                p.sensor[0] = v;
                if (p.workspace?.design_task && v <= 100)
                  p.workspace.design_task.sensor_mm[0] = v;
              })
            }
          />
          <Num
            label="目标传感器高度"
            value={project.sensor[1]}
            min={0.01}
            unit="mm"
            onChange={(v) =>
              edit((p) => {
                p.sensor[1] = v;
                if (p.workspace?.design_task && v <= 100)
                  p.workspace.design_task.sensor_mm[1] = v;
              })
            }
          />
        </div>
      </div>
      <FieldPanel project={project} result={result} edit={edit} />
      <div className="workflow-actions">
        <button
          disabled={disabled}
          onClick={() =>
            edit((p) => {
              const w = ensureWorkspace(p),
                o = w.optimization ?? defaultOptimization(p);
              if (!o.variables.some((v) => v.variable.kind === "aperture"))
                o.variables.push(
                  defaultSetting(p, { kind: "aperture", surface: p.stop }),
                );
              w.optimization = o;
              w.constraints = c;
            })
          }
        >
          将光阑半径加入优化变量
        </button>
        <button
          disabled={disabled}
          onClick={() => {
            if (!project.workspace?.constraints) set(c);
            optimize();
          }}
        >
          按目标生成优化候选
        </button>
      </div>
      <p className="hint">
        可在右侧专业模式编辑变量边界。目标或机械条件未满足时，候选仍会禁止采纳；优化不自动更换材料。
      </p>
      {error && <p className="error">{error}</p>}
      {checkedKey === key && checks.length > 0 ? (
        <ConstraintTable
          label="当前设计约束检查"
          checks={[
            ...checks,
            {
              name: "failed_rays",
              surface: null,
              actual: result
                ? result.spots.reduce((sum, s) => sum + s.failed, 0)
                : null,
              limit: 0,
              relation: "max",
              passed: !!result && result.spots.every((s) => s.failed === 0),
              reason: result
                ? `当前几何预览 ${result.sampling}×${result.sampling}，全部配置视场与波长；不代替候选的 33×33 复核。`
                : "等待当前处方的几何追迹；尚未确认通光。",
            },
          ]}
        />
      ) : (
        <p role="status">
          {error ? "当前设计未完成约束检查。" : "正在检查当前目标与几何约束…"}
        </p>
      )}
    </div>
  );
}
