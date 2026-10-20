import { useState } from "react";
import Modal from "./Modal";
import type { Project } from "./types";
import {
  applyGlass,
  glassNumbers,
  glassDescription,
  glasses,
} from "./materials";
export default function MaterialLibrary({
  project,
  selected,
  edit,
}: {
  project: Project;
  selected: number;
  edit: (fn: (p: Project) => void) => void;
}) {
  const targets = project.surfaces
    .map((s, i) => ({ s, i }))
    .filter(({ s, i }) => s.material !== 0 && i < project.surfaces.length - 1);
  const [open, setOpen] = useState(false),
    [target, setTarget] = useState(0),
    [query, setQuery] = useState(""),
    [ndMin, setNdMin] = useState(""),
    [ndMax, setNdMax] = useState(""),
    [vdMin, setVdMin] = useState(""),
    [vdMax, setVdMax] = useState(""),
    [problem, setProblem] = useState("");
  const rows = glasses.filter((m) => {
    const { nd, vd } = glassNumbers(m);
    return (
      m.name.toLowerCase().includes(query.toLowerCase()) &&
      nd !== null &&
      vd !== null &&
      (!ndMin || nd >= Number(ndMin)) &&
      (!ndMax || nd <= Number(ndMax)) &&
      (!vdMin || vd >= Number(vdMin)) &&
      (!vdMax || vd <= Number(vdMax))
    );
  });
  return (
    <>
      <button
        disabled={!targets.length}
        className="material-library-button"
        onClick={() => {
          setTarget(
            targets.some((t) => t.i === selected)
              ? selected
              : (targets.find((t) => t.i >= selected - 1)?.i ?? targets[0].i),
          );
          setProblem("");
          setOpen(true);
        }}
      >
        查找真实玻璃材料…
      </button>
      {open && (
        <Modal wide title="离线玻璃材料库" onCancel={() => setOpen(false)}>
          <div className="glass-library">
            <label className="text-label">
              替换玻璃实体
              <select
                aria-label="替换玻璃实体"
                value={target}
                onChange={(e) => setTarget(Number(e.target.value))}
              >
                {targets.map(({ s, i }) => (
                  <option key={i} value={i}>
                    S{i + 1}–S{i + 2} {project.materials[s.material].name}
                  </option>
                ))}
              </select>
            </label>
            <label className="text-label">
              搜索名称
              <input
                aria-label="玻璃名称搜索"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
            </label>
            <div className="glass-filters">
              {[
                ["nd 下限", ndMin, setNdMin],
                ["nd 上限", ndMax, setNdMax],
                ["Vd 下限", vdMin, setVdMin],
                ["Vd 上限", vdMax, setVdMax],
              ].map(([label, value, set]) => (
                <label key={label as string}>
                  {label as string}
                  <input
                    aria-label={label as string}
                    type="number"
                    step="any"
                    value={value as string}
                    onChange={(e) =>
                      (set as (v: string) => void)(e.target.value)
                    }
                  />
                </label>
              ))}
            </div>
            <p>
              {rows.length} / {glasses.length} 种 · nd 为 d 线折射率，Vd =
              (nd−1)/(nF−nC)。标称 293 K；不计算温漂与吸收。
            </p>
            <details>
              <summary>查看当前筛选的折射率–阿贝数分布</summary>
              <svg
                viewBox="0 0 640 230"
                role="img"
                aria-label="玻璃分布图：横轴阿贝数，纵轴 d 线折射率"
                style={{ width: "100%", maxHeight: 230 }}
              >
                <path d="M55 20V190H615" fill="none" stroke="currentColor" />
                <text x="260" y="225" fill="currentColor">
                  阿贝数 Vd → 低色散
                </text>
                <text x="5" y="18" fill="currentColor">
                  nd
                </text>
                {[20, 40, 60, 80].map((v) => (
                  <text
                    key={v}
                    x={55 + (v - 20) * 8}
                    y={210}
                    fill="currentColor"
                  >
                    {v}
                  </text>
                ))}
                {[1.5, 1.7, 1.9].map((v) => (
                  <text
                    key={v}
                    x={10}
                    y={190 - (v - 1.45) * 320}
                    fill="currentColor"
                  >
                    {v}
                  </text>
                ))}
                {rows.map((m) => {
                  const { nd, vd } = glassNumbers(m);
                  return (
                    <circle
                      key={m.name}
                      cx={55 + (vd! - 20) * 8}
                      cy={190 - (nd! - 1.45) * 320}
                      r={4}
                      fill="#cde4aa"
                    >
                      <title>
                        {m.name} · nd {nd?.toFixed(5)} / Vd {vd?.toFixed(2)}
                      </title>
                    </circle>
                  );
                })}
              </svg>
              <p className="hint">
                按同一坐标比较材料，悬停查看名称；相近 nd/Vd
                不代表部分色散、温度与制造性能相同。表内 P(g,F)=(ng−nF)/(nF−nC)
                用于比较二级光谱，不能独自证明复消色差。
              </p>
            </details>
            <div className="scroll-table">
              <table>
                <thead>
                  <tr>
                    <th>玻璃</th>
                    <th>nd / Vd / P(g,F)</th>
                    <th>有效波段</th>
                    <th>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((m) => {
                    const { nd, vd, pgf } = glassNumbers(m),
                      valid = project.wavelengths.every(
                        (w) => w.nm >= m.range_nm[0] && w.nm <= m.range_nm[1],
                      );
                    return (
                      <tr key={m.name}>
                        <td>
                          {m.name}
                          <small>{glassDescription(m)}</small>
                          <small>
                            <a href={m.url} target="_blank" rel="noreferrer">
                              数据与来源 · CC0
                            </a>
                          </small>
                        </td>
                        <td>
                          {nd?.toFixed(6)}
                          <small>{vd?.toFixed(3)}</small>
                          <small>{pgf?.toFixed(5) ?? "—"}</small>
                        </td>
                        <td>
                          {m.range_nm.join("–")} nm
                          {!valid && (
                            <small className="error">当前波长超出范围</small>
                          )}
                        </td>
                        <td>
                          <button
                            disabled={!valid}
                            onClick={() => {
                              try {
                                edit((p) => applyGlass(p, target, m));
                                setOpen(false);
                              } catch (e) {
                                setProblem(String(e));
                              }
                            }}
                          >
                            使用 {m.name}
                          </button>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
            {problem && (
              <p role="alert" className="error">
                {problem}
              </p>
            )}
            <p className="hint">
              系数来自 refractiveindex.info 的 CC0 数据快照，原始来源为 SCHOTT
              2017
              目录。应用后写入项目，不依赖联网；仅替换所选实体，不更改共用旧材料的其他镜片。玻璃变更后需要重新对焦和检查几何。
            </p>
            <div className="dialog-actions">
              <button onClick={() => setOpen(false)}>关闭</button>
            </div>
          </div>
        </Modal>
      )}
    </>
  );
}
