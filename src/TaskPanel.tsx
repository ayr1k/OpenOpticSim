import { useState, useSyncExternalStore } from "react";
import { tasks } from "./taskQueue";
export default function TaskPanel() {
  const rows = useSyncExternalStore(tasks.subscribe, tasks.snapshot);
  const [paused, setPaused] = useState(false);
  const status = {
    queued: "排队",
    running: "运行中",
    done: "完成",
    failed: "失败",
    cancelled: "已取消",
  };
  return (
    <details className="workflow-panel">
      <summary>
        计算任务 · {rows.filter((r) => r.state === "running").length} 运行 /{" "}
        {rows.filter((r) => r.state === "queued").length} 等待
      </summary>
      <label>
        同时运行任务数{" "}
        <select
          defaultValue="1"
          onChange={(e) => tasks.setLimit(Number(e.target.value))}
        >
          <option value="1">1 · 优先响应与内存</option>
          <option value="2">2 · 并行独立分析</option>
        </select>
      </label>
      <button
        onClick={() => {
          tasks.setPaused(!paused);
          setPaused(!paused);
        }}
      >
        {paused ? "恢复队列" : "暂停启动新任务"}
      </button>
      <p className="hint">
        暂停队列不打断正在运行的求解。单个光学任务仍使用引擎内部并行。取消运行任务后，等待计算线程退出再释放名额。最近
        20 项记录仅保留在本次会话。
      </p>
      <ul>
        {rows.map((r) => (
          <li key={r.id}>
            {r.label} · {status[r.state]} {r.detail}{" "}
            {(r.state === "running" || r.state === "queued") && (
              <button onClick={() => tasks.cancel(r.id)}>取消任务</button>
            )}
          </li>
        ))}
      </ul>
    </details>
  );
}
