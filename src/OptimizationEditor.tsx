import { useState } from "react";
import type { OptimizationOptions, Project, Variable } from "./types";
import Modal from "./Modal";
import { Num } from "./Num";
import {
  defaultSetting,
  fieldWeights,
  optimizationError,
  presetLabels,
  variableKey,
  variableLabel,
  variableUnit,
  variableValue,
} from "./optimization";
import { copy } from "./project";
import QualityEditor from "./QualityEditor";
import { fieldLabel } from "./fields";
export default function OptimizationEditor({
  project,
  value,
  onChange,
  resolvedFields,
}: {
  project: Project;
  value: OptimizationOptions;
  onChange: (v: OptimizationOptions) => void;
  resolvedFields?: number[];
}) {
  const [open, setOpen] = useState(false),
    [draft, setDraft] = useState(value),
    [surface, setSurface] = useState(0),
    [kind, setKind] = useState("curvature");
  const error = optimizationError(project, draft),
    active = value.variables.filter((v) => !v.locked).length;
  const weights = fieldWeights(resolvedFields ?? project.fields, value.preset);
  return (
    <>
      <QualityEditor project={project} value={value} onChange={onChange} />
      <label className="text-label">
        画质目标预设
        <select
          aria-label="画质目标预设"
          value={value.preset}
          onChange={(e) =>
            onChange({
              ...value,
              preset: e.target.value as OptimizationOptions["preset"],
            })
          }
        >
          {Object.entries(presetLabels).map(([k, v]) => (
            <option key={k} value={k}>
              {v}
            </option>
          ))}
        </select>
      </label>
      <p className="hint">
        多色 RMS 均方目标；视场权重{" "}
        {weights
          .map((w, i) => `${fieldLabel(project, i)} ${(100 * w).toFixed(1)}%`)
          .join(" / ")}
        。
        {!resolvedFields && project.field_spec
          ? "当前为初始权重预览，以实际视场求解后的报告为准。"
          : ""}
        通光仍检查全部视场。
      </p>
      <button
        onClick={() => {
          setDraft(copy(value));
          setSurface(
            Math.min(surface, Math.max(0, project.surfaces.length - 1)),
          );
          setOpen(true);
        }}
      >
        编辑多表面变量（{active} 个参与）
      </button>
      {!!optimizationError(project, value) && (
        <p className="error">{optimizationError(project, value)}</p>
      )}
      {open && (
        <Modal wide title="优化变量与上下限" onCancel={() => setOpen(false)}>
          <div className="optimization-editor">
            <p>
              锁定的行保留配置且不参与优化。空白上下限表示无限制；曲率使用
              1/mm。修改仅在点击“应用变量设置”后保存。
            </p>
            <div className="workflow-actions">
              <label>
                表面
                <select
                  aria-label="新增变量表面"
                  value={surface}
                  onChange={(e) => setSurface(Number(e.target.value))}
                >
                  {project.surfaces.map((s, i) => (
                    <option key={i} value={i}>
                      S{i + 1} {s.name}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                参数
                <select
                  aria-label="新增变量参数"
                  value={kind}
                  onChange={(e) => setKind(e.target.value)}
                >
                  {[
                    ["curvature", "曲率"],
                    ["thickness", "面后间隔 / 像距"],
                    ["aperture", "光阑半径（仅光阑面）"],
                    ["0", "A4"],
                    ["1", "A6"],
                    ["2", "A8"],
                    ["3", "A10"],
                  ].map(([k, l]) => (
                    <option key={k} value={k}>
                      {l}
                    </option>
                  ))}
                </select>
              </label>
              <button
                disabled={
                  !project.surfaces.length || draft.variables.length >= 100
                }
                onClick={() => {
                  const variable: Variable =
                    kind === "curvature" ||
                    kind === "thickness" ||
                    kind === "aperture"
                      ? { kind, surface }
                      : { kind: "asphere", surface, coefficient: Number(kind) };
                  if (
                    draft.variables.some(
                      (s) => variableKey(s.variable) === variableKey(variable),
                    )
                  )
                    return;
                  setDraft({
                    ...draft,
                    variables: [
                      ...draft.variables,
                      defaultSetting(project, variable),
                    ],
                  });
                }}
              >
                添加变量
              </button>
            </div>
            <div className="scroll-table">
              <table>
                <thead>
                  <tr>
                    <th>参数 / 单位</th>
                    <th>当前值</th>
                    <th>下限</th>
                    <th>上限</th>
                    <th>锁定</th>
                    <th>移除</th>
                  </tr>
                </thead>
                <tbody>
                  {draft.variables.map((s, i) => (
                    <tr key={variableKey(s.variable)}>
                      <td>
                        {variableLabel(s.variable)}
                        <small>{variableUnit(s.variable)}</small>
                      </td>
                      <td>
                        {variableValue(project, s.variable)?.toPrecision(9) ??
                          "面已不存在"}
                      </td>
                      {(["min", "max"] as const).map((key) => (
                        <td key={key}>
                          <input
                            aria-label={`${variableLabel(s.variable)} ${key === "min" ? "下限" : "上限"}`}
                            type="number"
                            step="any"
                            value={s[key] ?? ""}
                            placeholder="无限制"
                            onChange={(e) => {
                              const v =
                                e.target.value === ""
                                  ? null
                                  : Number(e.target.value);
                              setDraft({
                                ...draft,
                                variables: draft.variables.map((s, j) =>
                                  j === i ? { ...s, [key]: v } : s,
                                ),
                              });
                            }}
                          />
                        </td>
                      ))}
                      <td>
                        <input
                          type="checkbox"
                          aria-label={`锁定 ${variableLabel(s.variable)}`}
                          checked={s.locked}
                          onChange={(e) =>
                            setDraft({
                              ...draft,
                              variables: draft.variables.map((s, j) =>
                                j === i
                                  ? { ...s, locked: e.target.checked }
                                  : s,
                              ),
                            })
                          }
                        />
                      </td>
                      <td>
                        <button
                          aria-label={`移除 ${variableLabel(s.variable)}`}
                          onClick={() =>
                            setDraft({
                              ...draft,
                              variables: draft.variables.filter(
                                (_, j) => j !== i,
                              ),
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
            </div>
            <Num
              label="最大优化迭代数"
              min={1}
              max={100}
              step={1}
              value={draft.max_iterations}
              onChange={(n) =>
                setDraft({ ...draft, max_iterations: Math.round(n) })
              }
            />
            <p>
              {draft.variables.filter((s) => !s.locked).length} / 32
              个参与优化；其余表面参数保持固定。
            </p>
            {error && (
              <p className="error" role="alert">
                {error}
              </p>
            )}
            <div className="dialog-actions">
              <button onClick={() => setOpen(false)}>取消</button>
              <button
                className="primary"
                disabled={!!error}
                onClick={() => {
                  onChange(draft);
                  setOpen(false);
                }}
              >
                应用变量设置
              </button>
            </div>
          </div>
        </Modal>
      )}
    </>
  );
}
