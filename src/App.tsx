import MotionPanel from "./MotionPanel";
import { synchronizeMotion } from "./motion";
import { lazy, Suspense, useEffect, useRef, useState } from "react";
import {
  Aperture,
  ArrowDownToLine,
  ArrowUpFromLine,
  ChevronRight,
  Copy,
  Focus,
  PanelLeft,
  PanelRight,
  LoaderCircle,
  Plus,
  Redo2,
  RotateCcw,
  Save,
  Settings2,
  Sparkles,
  Trash2,
  Undo2,
  X,
  BookOpen,
  Search,
} from "lucide-react";
import {
  compute,
  examples,
  native,
  openNativeProject,
  saveNativeProject,
  exportReport,
} from "./engine";
import type {
  Analysis,
  Candidate,
  Geometry,
  Project,
  Surface,
  OptimizationProgress,
  Ray,
} from "./types";
import { copy, download, fromCsv, projectFromJson, toCsv } from "./project";
import { Layout, Spots } from "./Plots";
import WavePanel from "./WavePanel";
import PreviewPanel from "./PreviewPanel";
import MaterialEditor from "./MaterialEditor";
import RayDiagnostics from "./RayDiagnostics";
import { diagnostic, explainError } from "./errors";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { insertLens, lensAt, templates, type LensKind } from "./lenses";
import LensEditor from "./LensEditor";
import NewProject from "./NewProject";
import Modal from "./Modal";
import GeometryPanel from "./GeometryPanel";
import { Num } from "./Num";
import StructurePanel from "./StructurePanel";
import { defaultLayout, readLayout, saveLayout } from "./workspaceLayout";
import FocusPanel from "./FocusPanel";
import AperturePanel from "./AperturePanel";
import "./design.css";
import GoalPanel from "./GoalPanel";
import Coverage from "./Coverage";
import PupilPanel from "./PupilPanel";
import WorkspaceNav from "./WorkspaceNav";
import { helpFor } from "./navigation";
import "./studio.css";
import GroupPanel from "./GroupPanel";
import { opticalKey, candidateKey } from "./design";
import VersionPanel from "./VersionPanel";
import {
  ConstraintEditor,
  ConstraintTable,
  adoptionBlockers,
} from "./ConstraintEditor";
import OptimizationEditor from "./OptimizationEditor";
import OptimizationReportView, { ProgressView } from "./OptimizationReport";
import MaterialLibrary from "./MaterialLibrary";
import { defaultOptimization, optimizationError } from "./optimization";
import {
  addVersion,
  defaultConstraints,
  ensureWorkspace,
  prescription,
  restoreVersion,
} from "./versions";
const fmt = (v: number | null | undefined, d = 2) =>
  v == null || !Number.isFinite(v) ? "—" : v.toFixed(d);
const rms = (a: Analysis | null) =>
  a && a.fields.every((f) => f.polychromatic_rms_um !== null)
    ? Math.sqrt(
        a.fields.reduce((sum, f) => sum + f.polychromatic_rms_um! ** 2, 0) /
          a.fields.length,
      )
    : null;
const HelpCenter = lazy(() => import("./HelpCenter"));
const CommandSearch = lazy(() => import("./CommandSearch"));
const DrawingPanel = lazy(() => import("./DrawingPanel"));
export default function App() {
  const [helpTopic, setHelpTopic] = useState<string | null>(null),
    [showSearch, setShowSearch] = useState(false),
    [inspectorTab, setInspectorTab] = useState("element");
  const [workspaceLayout, setWorkspaceLayout] = useState(readLayout),
    [layoutResetEpoch, setLayoutResetEpoch] = useState(0),
    [showNew, setShowNew] = useState(false),
    [analysisEpoch, setAnalysisEpoch] = useState(0),
    [documentEpoch, setDocumentEpoch] = useState(0),
    [geometry, setGeometry] = useState<Geometry | null>(null),
    [lensKind, setLensKind] = useState<LensKind>("biconvex"),
    [filePath, setFilePath] = useState<string | null>(null),
    [savedKey, setSavedKey] = useState<string | null>(null),
    [pending, setPending] = useState(false),
    [saving, setSaving] = useState(false),
    [saveProblem, setSaveProblem] = useState("");
  const pendingAction = useRef<(() => void) | null>(null);
  const fileMenu = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    const close = (e: PointerEvent) => {
      if (e.target instanceof Node && !fileMenu.current?.contains(e.target))
        fileMenu.current?.removeAttribute("open");
    };
    const escape = (e: KeyboardEvent) => {
      if (e.key === "Escape" && fileMenu.current?.open) {
        fileMenu.current.removeAttribute("open");
        fileMenu.current.querySelector("summary")?.focus();
      }
    };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", close);
      document.removeEventListener("keydown", escape);
    };
  }, []);
  const pendingDiscard = useRef<(() => void) | null>(null);
  const [library, setLibrary] = useState<Project[]>([]),
    [project, setProject] = useState<Project | null>(null),
    [result, setResult] = useState<Analysis | null>(null),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(""),
    [selected, setSelected] = useState(1),
    [mode, setMode] = useState("basic"),
    [tab, setTab] = useState("spots"),
    [candidate, setCandidate] = useState<Candidate | null>(null),
    [history, setHistory] = useState<Project[]>([]),
    [future, setFuture] = useState<Project[]>([]),
    [optimizationProgress, setOptimizationProgress] = useState<
      OptimizationProgress[]
    >([]),
    [notice, setNotice] = useState("");
  const [selectedRay, setSelectedRay] = useState<Ray | null>(null);
  useEffect(() => setSelectedRay(null), [project && opticalKey(project)]);
  const controller = useRef<AbortController | null>(null),
    revision = useRef(0),
    file = useRef<HTMLInputElement>(null),
    ready = useRef(false),
    scrubbing = useRef(false);
  const rightScroll = useRef<HTMLDivElement>(null);
  useEffect(() => saveLayout(workspaceLayout), [workspaceLayout]);
  const inspectorKey =
    mode === "basic" && project
      ? (lensAt(project, selected)?.start ?? selected)
      : selected;
  useEffect(() => {
    if (rightScroll.current) rightScroll.current.scrollTop = 0;
  }, [documentEpoch, inspectorKey, mode, inspectorTab]);
  useEffect(() => setInspectorTab("element"), [documentEpoch, selected]);
  const navigate = (next: string) => {
    setTab(next);
    requestAnimationFrame(() =>
      document
        .getElementById("analysis-workbench")
        ?.scrollIntoView({ block: "start", behavior: "instant" }),
    );
  };
  const latestProject = useRef<Project | null>(project);
  const documentStamp = useRef(0),
    sourceDocumentStamp = documentStamp.current;
  latestProject.current = project;
  const installProject = (p: Project) => {
    if (
      latestProject.current &&
      candidateKey(latestProject.current) !== candidateKey(p)
    ) {
      controller.current?.abort();
      revision.current++;
    }
    latestProject.current = p;
    setProject(p);
  };
  const receiveCandidate = (c: Candidate, source: Project, stamp: number) => {
    if (
      documentStamp.current === stamp &&
      latestProject.current &&
      candidateKey(latestProject.current) === candidateKey(source)
    )
      setCandidate(c);
    else setNotice("设计或约束已改变，旧候选未回填，请重新计算。");
  };
  const beginScrub = () => {
    if (!project || scrubbing.current) return;
    scrubbing.current = true;
    setHistory((h) => [...h.slice(-79), copy(project)]);
    setFuture([]);
    setCandidate(null);
  };
  const endScrub = () => {
    if (!scrubbing.current) return;
    scrubbing.current = false;
    if (latestProject.current) installProject(copy(latestProject.current));
    setAnalysisEpoch((v) => v + 1);
  };
  const edit = (fn: (p: Project) => void) => {
    const current = latestProject.current;
    if (!current) return;
    const p = copy(current);
    fn(p);
    synchronizeMotion(current, p);
    if (p.workspace?.optimization?.quality) {
      const floors = p.workspace.optimization.quality.max_rms_um;
      if (floors.length && floors.length !== p.fields.length) {
        p.workspace.optimization.quality.max_rms_um = [];
        setNotice(
          "视场数量已改变，逐视场 RMS 底线已清空，请重新确认；全局保护规则继续保留。",
        );
      }
    }
    if (p.workspace?.optimization?.quality || p.workspace?.optimization?.staged)
      p.schema_version = Math.max(3, p.schema_version);
    if (
      p.surfaces.length !== current.surfaces.length &&
      p.workspace?.groups?.length
    ) {
      p.workspace.groups = [];
      setNotice("表面数量已改变，镜组定义已清空，请重新编组。");
    }
    if (
      p.surfaces.length !== current.surfaces.length &&
      p.workspace?.optimization
    ) {
      p.workspace.optimization.variables = [];
      setNotice("表面数量已改变，优化变量表已清空，请重新选择以避免引用错面。");
    }
    setHistory((h) => [...h.slice(-79), copy(current)]);
    setFuture([]);
    installProject(p);
    setCandidate(null);
  };
  const replace = (p: Project) => {
    if (project) setHistory((h) => [...h.slice(-79), copy(project)]);
    setFuture([]);
    installProject(copy(p));
    setSelected(Math.min(1, p.surfaces.length - 1));
    setCandidate(null);
  };
  const update = (patch: Partial<Surface>) =>
    edit((p) => Object.assign(p.surfaces[selected], patch));
  const dirty = !!project && JSON.stringify(project) !== savedKey;
  const requestAction = (
    action: () => void,
    onDiscard: (() => void) | null = null,
  ) => {
    if (saving || pending) return;
    if (dirty) {
      setSaveProblem("");
      pendingAction.current = action;
      pendingDiscard.current = onDiscard;
      setPending(true);
    } else action();
  };
  const continueAction = () => {
    const action = pendingAction.current;
    pendingAction.current = null;
    pendingDiscard.current = null;
    setPending(false);
    action?.();
  };
  const loadDocument = (
    p: Project,
    path: string | null = null,
    saved = false,
  ) => {
    controller.current?.abort();
    revision.current++;
    documentStamp.current++;
    setAnalysisEpoch((v) => v + 1);
    setDocumentEpoch((v) => v + 1);
    const next = copy(p);
    latestProject.current = next;
    setProject(next);
    setHistory([]);
    setFuture([]);
    setCandidate(null);
    setSelected(p.surfaces.length > 1 ? 1 : 0);
    setFilePath(path);
    setSavedKey(saved ? JSON.stringify(p) : null);
    setTab("geometry");
    setNotice("");
  };
  const addLens = () => {
    try {
      edit((p) => setSelected(insertLens(p, lensKind)));
      setTab("geometry");
    } catch (e) {
      setError(explainError(e));
    }
  };
  const addSurface = () =>
    edit((p) => {
      const last = p.surfaces.pop();
      if (!last) {
        p.surfaces.push({
          name: "光阑",
          curvature: 0,
          conic: 0,
          asphere: [0, 0, 0, 0],
          thickness: 50,
          semi_aperture: 4,
          material: 0,
        });
        p.stop = 0;
        return;
      }
      const half = last.thickness / 2;
      last.thickness = half;
      p.surfaces.push(last, {
        name: "新平面",
        curvature: 0,
        conic: 0,
        asphere: [0, 0, 0, 0],
        thickness: half,
        semi_aperture: 12,
        material: 0,
      });
    });
  const closeHandler = useRef<() => void>(() => {});
  closeHandler.current = () =>
    requestAction(
      () => {
        void getCurrentWindow()
          .destroy()
          .catch((e) => {
            setError(explainError(e));
            try {
              if (project)
                localStorage.setItem("oos-autosave", JSON.stringify(project));
            } catch {}
          });
      },
      () => {
        try {
          if (savedKey) localStorage.setItem("oos-autosave", savedKey);
          else localStorage.removeItem("oos-autosave");
        } catch {}
      },
    );
  useEffect(() => {
    if (!native) return;
    let disposed = false,
      unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onCloseRequested((e) => {
        e.preventDefault();
        closeHandler.current();
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
  useEffect(() => {
    const fn = (e: BeforeUnloadEvent) => {
      if (dirty) {
        e.preventDefault();
        e.returnValue = "";
      }
    };
    if (!native) window.addEventListener("beforeunload", fn);
    return () => window.removeEventListener("beforeunload", fn);
  }, [dirty]);
  const undo = () => {
    if (!project || !history.length) return;
    setFuture((f) => [copy(project), ...f]);
    installProject(copy(history[history.length - 1]));
    setHistory((h) => h.slice(0, -1));
    setCandidate(null);
  };
  const redo = () => {
    if (!project || !future.length) return;
    setHistory((h) => [...h, copy(project)]);
    installProject(copy(future[0]));
    setFuture((f) => f.slice(1));
    setCandidate(null);
  };
  useEffect(() => {
    let alive = true;
    examples()
      .then((items) => {
        if (!alive) return;
        setLibrary(items);
        let p = items[0];
        try {
          const saved = localStorage.getItem("oos-autosave");
          if (saved) p = projectFromJson(saved);
        } catch {}
        latestProject.current = p;
        setProject(p);
        ready.current = true;
      })
      .catch((e) => {
        if (alive) setError(explainError(e));
      });
    return () => {
      alive = false;
    };
  }, []);
  useEffect(() => {
    if (!project) return;
    if (ready.current) {
      try {
        localStorage.setItem("oos-autosave", JSON.stringify(project));
      } catch {
        setNotice("本地自动恢复不可用，请使用“保存项目”。");
      }
    }
  }, [project]);
  useEffect(() => {
    if (!project) return;
    setSelected((s) => Math.max(0, Math.min(s, project.surfaces.length - 1)));
    controller.current?.abort();
    const c = new AbortController();
    controller.current = c;
    const rev = ++revision.current;
    setError("");
    setGeometry(null);
    setOptimizationProgress([]);
    if (!project.surfaces.length) {
      setResult(null);
      setBusy("");
      return () => c.abort();
    }
    setBusy("预览中");
    setResult(null);
    const timer = setTimeout(async () => {
      try {
        const g = await compute<Geometry>(
          `${rev}-geometry`,
          "geometry",
          project,
          c.signal,
        );
        if (rev !== revision.current || c.signal.aborted) return;
        setGeometry(g);
        const low = await compute<Analysis>(
          `${rev}-preview`,
          "analyze",
          project,
          c.signal,
          { sampling: 7 },
        );
        if (rev !== revision.current || c.signal.aborted) return;
        setResult(low);
        if (scrubbing.current) {
          setBusy("拖动预览");
          return;
        }
        setBusy("精细计算中");
        const high = await compute<Analysis>(
          `${rev}-full`,
          "analyze",
          project,
          c.signal,
          { sampling: 25 },
        );
        if (rev === revision.current && !c.signal.aborted) {
          setResult(high);
          setBusy("");
        }
      } catch (e) {
        if (rev === revision.current && !c.signal.aborted) {
          setError(explainError(e));
          setBusy("");
        }
      }
    }, 180);
    return () => {
      clearTimeout(timer);
      c.abort();
    };
  }, [
    analysisEpoch,
    project &&
      JSON.stringify([
        prescription(project),
        project.workspace?.constraints,
        project.workspace?.optimization,
      ]),
  ]);
  useEffect(() => {
    const fn = (e: KeyboardEvent) => {
      if (e.key === "F1") {
        e.preventDefault();
        if (!pending && !showNew && !showSearch) setHelpTopic(helpFor(tab));
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        if (!pending && !showNew && !helpTopic) setShowSearch(true);
        return;
      }
      if (
        (pending || showNew || saving || helpTopic || showSearch) &&
        (e.ctrlKey || e.metaKey) &&
        ["s", "n", "o", "z"].includes(e.key.toLowerCase())
      ) {
        e.preventDefault();
        return;
      }
      if (e.ctrlKey || e.metaKey) {
        const k = e.key.toLowerCase();
        if (["s", "n", "o"].includes(k)) {
          e.preventDefault();
          if (pending || showNew || saving) return;
          if (k === "s") void saveFile(false, e.shiftKey);
          if (k === "n") requestAction(() => setShowNew(true));
          if (k === "o") requestAction(() => void openFile());
          return;
        }
      }
      if (
        (e.ctrlKey || e.metaKey) &&
        e.key.toLowerCase() === "z" &&
        !(e.target instanceof HTMLInputElement)
      ) {
        e.preventDefault();
        e.shiftKey ? redo() : undo();
      }
    };
    window.addEventListener("keydown", fn);
    return () => window.removeEventListener("keydown", fn);
  });
  const cancel = () => {
    controller.current?.abort();
    revision.current++;
    setBusy("");
    setOptimizationProgress([]);
    setNotice("计算已取消；修改参数或点击“重新分析”后继续。");
  };
  async function run(operation: "autofocus" | "optimize") {
    if (!project) return;
    const optimization =
      project.workspace?.optimization ?? defaultOptimization(project, selected);
    if (operation === "optimize") {
      const problem = optimizationError(project, optimization);
      if (problem || !optimization.variables.some((v) => !v.locked)) {
        setError(problem || "请添加至少一个未锁定变量");
        return;
      }
    }
    controller.current?.abort();
    const c = new AbortController();
    controller.current = c;
    const rev = ++revision.current;
    setError("");
    setCandidate(null);
    setOptimizationProgress([]);
    setBusy(operation === "autofocus" ? "搜索最佳像面" : "优化候选方案");
    try {
      const cnd = await compute<Candidate>(
        `${rev}`,
        operation,
        project,
        c.signal,
        {
          constraints:
            project.workspace?.constraints ??
            defaultConstraints(project, result?.paraxial.efl ?? 50),
          optimization: operation === "optimize" ? optimization : undefined,
        },
        operation === "optimize"
          ? (progress) => {
              if (rev === revision.current && !c.signal.aborted)
                setOptimizationProgress((h) => [...h, progress]);
            }
          : undefined,
      );
      if (rev === revision.current && !c.signal.aborted) {
        if (
          operation === "optimize" &&
          (!project.workspace?.constraints || !project.workspace.optimization)
        ) {
          const constraints =
            project.workspace?.constraints ??
            defaultConstraints(project, result?.paraxial.efl ?? 50);
          // Store the exact settings with the document; no optical edit is made.
          edit((p) => {
            ensureWorkspace(p).constraints = constraints;
            ensureWorkspace(p).optimization = optimization;
          });
        }
        setCandidate(cnd);
        setOptimizationProgress([]);
      }
    } catch (e) {
      if (!c.signal.aborted && rev === revision.current)
        setError(explainError(e));
    } finally {
      if (rev === revision.current) setBusy("");
    }
  }
  async function importFile(f: File, path: string | null = null) {
    try {
      if (f.size > 4_000_000) throw Error("项目文件不能超过 4 MB");
      const text = await f.text();
      const p = f.name.endsWith(".csv") ? fromCsv(text) : projectFromJson(text);
      // Geometrically invalid drafts must remain editable after reopening.
      loadDocument(
        p,
        f.name.endsWith(".csv") ? null : path,
        !f.name.endsWith(".csv"),
      );
      setNotice("项目已打开；几何问题可在编辑器中修正。");
    } catch (e) {
      setError(explainError(e));
    }
  }
  async function openFile() {
    if (!native) {
      file.current?.click();
      return;
    }
    try {
      const f = await openNativeProject();
      if (f) await importFile(new File([f.content], f.name), f.path);
    } catch (e) {
      setError(explainError(e));
    }
  }
  async function saveFile(csv = false, saveAs = false): Promise<boolean> {
    if (!project || saving) return false;
    setSaving(true);
    setSaveProblem("");
    const name = project.name + (csv ? ".csv" : ".oos.json");
    const content = csv ? toCsv(project) : JSON.stringify(project, null, 2);
    try {
      if (new TextEncoder().encode(content).length > 4_000_000)
        throw Error("项目文件超过 4 MB，请减少版本数量或材料记录后保存。");
      if (native) {
        const saved = await saveNativeProject(
          name,
          content,
          csv,
          csv || saveAs ? null : filePath,
        );
        setNotice(saved ? "项目已保存，包含材料快照。" : "已取消保存。");
        if (!saved) {
          setSaveProblem("已取消保存，当前项目仍保持打开。");
          return false;
        }
        if (!csv) {
          setFilePath(saved);
          setSavedKey(JSON.stringify(project));
        }
      } else {
        download(
          name,
          content,
          csv ? "text/csv;charset=utf-8" : "application/json",
        );
        setNotice("已发起项目下载，包含材料快照。");
        if (!csv) setSavedKey(JSON.stringify(project));
      }
      return true;
    } catch (e) {
      setError(explainError(e));
      setSaveProblem(explainError(e));
      return false;
    } finally {
      setSaving(false);
    }
  }
  if (!project)
    return (
      <div className="startup">
        <Aperture size={42} />
        <h1>OpenOpticSim</h1>
        <p>{error || "连接本地 Rust 光学引擎…"}</p>
        <button onClick={() => window.location.reload()}>重试连接</button>
        {import.meta.env.DEV && (
          <code>cargo build -p oos-cli　→　pnpm dev</code>
        )}
      </div>
    );
  const s = project.surfaces[selected] ?? project.surfaces[0];
  const selectedLens = lensAt(project, selected);
  const issues = result?.diagnostics ?? [];
  return (
    <div className="app">
      <header>
        <div className="brand">
          <Aperture size={27} />
          <div>
            OpenOpticSim <small>光学工作室</small>
          </div>
          <span className="alpha">1.2</span>
        </div>
        <div className="toolbar">
          <button
            disabled={saving}
            onClick={() => requestAction(() => setShowNew(true))}
          >
            <Plus size={15} />
            新建项目
          </button>
          <button
            disabled={saving}
            onClick={() => requestAction(() => void openFile())}
          >
            <ArrowUpFromLine size={15} />
            打开
          </button>
          <button disabled={saving} onClick={() => void saveFile()}>
            <Save size={15} />
            保存项目
          </button>
          <details ref={fileMenu} className="file-menu">
            <summary>文件与导出</summary>
            <div>
              <button
                disabled={saving}
                onClick={(e) => {
                  e.currentTarget.closest("details")?.removeAttribute("open");
                  void saveFile(false, true);
                }}
              >
                另存为项目
              </button>
              <button
                disabled={saving}
                onClick={(e) => {
                  e.currentTarget.closest("details")?.removeAttribute("open");
                  void saveFile(true);
                }}
              >
                <ArrowDownToLine size={15} />
                导出处方 CSV
              </button>
              <button
                onClick={(e) => {
                  e.currentTarget.closest("details")?.removeAttribute("open");
                  navigate("drawing");
                }}
              >
                导出技术图纸 PDF
              </button>
            </div>
          </details>
          <button
            title="查找功能与帮助 Ctrl+K"
            onClick={() => setShowSearch(true)}
          >
            <Search size={16} />
            <span className="optional-label">查找功能</span>
          </button>
          <button title="帮助与教程 F1" onClick={() => setHelpTopic("start")}>
            <BookOpen size={16} />
            <span className="optional-label">帮助</span>
          </button>
          <input
            hidden
            ref={file}
            type="file"
            accept=".json,.csv"
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) void importFile(f);
              e.target.value = "";
            }}
          />
        </div>
      </header>
      <div className="projectbar">
        <div>
          <span className="breadcrumb">
            工作空间 <ChevronRight size={13} />
          </span>
          <input
            className="project-name"
            aria-label="项目名称"
            value={project.name}
            onChange={(e) =>
              edit((p) => {
                p.name = e.target.value;
              })
            }
          />
          <span className="local-dot" title={filePath ?? "尚未选择保存位置"}>
            {dirty ? "未保存修改" : "已保存"} · 自动恢复
          </span>
        </div>
        <div className="toolbar">
          <button
            title="撤销 Ctrl+Z"
            aria-label="撤销"
            disabled={!history.length}
            onClick={undo}
          >
            <Undo2 size={16} />
          </button>
          <button
            title="重做 Ctrl+Shift+Z"
            aria-label="重做"
            disabled={!future.length}
            onClick={redo}
          >
            <Redo2 size={16} />
          </button>
          <span className="separator" />
          <button
            aria-label={
              workspaceLayout.leftHidden ? "展开结构侧栏" : "收起结构侧栏"
            }
            title={workspaceLayout.leftHidden ? "展开结构侧栏" : "收起结构侧栏"}
            aria-expanded={!workspaceLayout.leftHidden}
            onClick={() =>
              setWorkspaceLayout((v) => ({ ...v, leftHidden: !v.leftHidden }))
            }
          >
            <PanelLeft size={16} />
          </button>
          <button
            aria-label={
              workspaceLayout.rightHidden ? "展开参数侧栏" : "收起参数侧栏"
            }
            title={
              workspaceLayout.rightHidden ? "展开参数侧栏" : "收起参数侧栏"
            }
            aria-expanded={!workspaceLayout.rightHidden}
            onClick={() =>
              setWorkspaceLayout((v) => ({ ...v, rightHidden: !v.rightHidden }))
            }
          >
            <PanelRight size={16} />
          </button>
          <button
            aria-label="恢复工作区布局"
            title="恢复工作区布局与列表比例"
            onClick={() => {
              setWorkspaceLayout({ ...defaultLayout });
              setLayoutResetEpoch((v) => v + 1);
            }}
          >
            <RotateCcw size={15} />
          </button>
          <span className="separator" />
          <button
            onClick={() => {
              try {
                edit((p) =>
                  addVersion(
                    p,
                    project,
                    `版本 ${(project.workspace?.versions.length ?? 0) + 1}`,
                  ),
                );
                navigate("versions");
                setNotice("已记录版本，请保存项目以保留版本库。");
              } catch (e) {
                setError(explainError(e));
              }
            }}
          >
            <Copy size={14} />
            记录版本
          </button>
          <button onClick={() => navigate("versions")}>版本对比</button>
        </div>
      </div>
      <div
        className="workspace"
        style={{
          gridTemplateColumns: `${workspaceLayout.leftHidden ? "0px" : "var(--left-width)"} minmax(0,1fr) ${workspaceLayout.rightHidden ? "0px" : "var(--right-width)"}`,
        }}
      >
        <aside
          className="left"
          aria-label="光学结构导航"
          hidden={workspaceLayout.leftHidden}
        >
          <StructurePanel
            project={project}
            library={library}
            selected={selected}
            select={setSelected}
            mode={mode}
            kind={lensKind}
            setKind={setLensKind}
            addLens={addLens}
            addSurface={addSurface}
            openExample={(p) => requestAction(() => loadDocument(p))}
            lensShare={workspaceLayout.lensShare}
            setLensShare={(lensShare) =>
              setWorkspaceLayout((v) => ({ ...v, lensShare }))
            }
            visible={!workspaceLayout.leftHidden}
            resetEpoch={layoutResetEpoch + documentEpoch}
          />
        </aside>
        <main aria-label="光路与分析工作区" tabIndex={0}>
          <WorkspaceNav
            tab={tab}
            onTab={navigate}
            onHelp={() => setHelpTopic(helpFor(tab))}
          />
          <div className="workspace-guide">
            <span>先定义目标，再检查通光与画质；调校结果以候选方式采纳。</span>
            <button onClick={() => setHelpTopic("start")}>
              新手教程 <ChevronRight size={13} />
            </button>
          </div>
          <div className="viewport">
            <div className="viewbar">
              <div>
                <span className="live-dot" />
                光路预览 <small>2D LAYOUT</small>
              </div>
              <button
                className="quiet"
                onClick={() => requestAction(() => loadDocument(library[0]))}
              >
                <RotateCcw size={13} />
                恢复单透镜
              </button>
            </div>
            <Layout
              key={documentEpoch}
              project={project}
              result={result}
              selected={selected}
              onSelect={setSelected}
              highlightedRay={selectedRay}
            />
            {!project.surfaces.length && (
              <div className="blank-guide">
                <h2>从第一枚镜片开始</h2>
                <p>
                  选择镜片类型并添加。会自动加入独立光阑和像面间隔；随后可编辑曲率、材料与机械边缘。
                </p>
                <button className="primary" onClick={addLens}>
                  添加{templates[lensKind]}
                </button>
              </div>
            )}
            <div className="legend">
              {project.wavelengths.map((w, i) => (
                <span key={i}>
                  <i
                    style={{
                      background:
                        w.nm < 530
                          ? "#9ebdfb"
                          : w.nm < 620
                            ? "#a9d990"
                            : "#efa77a",
                    }}
                  />
                  {fmt(w.nm, 1)} nm · 权重 {w.weight}
                </span>
              ))}
              <button
                onClick={() => {
                  setWorkspaceLayout((v) => ({ ...v, rightHidden: false }));
                  setInspectorTab("photography");
                }}
              >
                编辑光谱与摄影设置
              </button>
            </div>
          </div>
          <div className="metrics">
            {[
              ["有效焦距", fmt(result?.paraxial.efl), "mm"],
              ["近轴 F 数", fmt(result?.paraxial.f_number), "f /"],
              ["后焦距", fmt(result?.paraxial.bfl), "mm"],
              ["多色 RMS 光斑", fmt(rms(result)), "µm"],
            ].map(([label, value, unit]) => (
              <div key={label}>
                <span>{label}</span>
                <strong>
                  {value}
                  <small>{unit}</small>
                </strong>
              </div>
            ))}
          </div>
          {result && result.spots.some((s) => s.failed > 0) && (
            <div className="ray-warning" role="status">
              <strong>通光尚未确认</strong>
              <p>
                近轴 F 数不是已验证的可用光圈；RMS
                只统计有效光线，缺少波段时不显示多色结果。请查看瞄准与物理失败的分别统计。
              </p>
              <button onClick={() => setTab("aberration")}>
                查看追迹失败明细
              </button>
            </div>
          )}
          <Coverage value={result?.coverage} compact />
          {result &&
            project.object_distance === null &&
            (result.paraxial.bfl ?? 1) <= 0 && (
              <div className="ray-warning" role="alert">
                <strong>当前为近轴虚像，后方自动对焦不能修复</strong>
                <p>{diagnostic(`VirtualImage|${result.paraxial.bfl}`)}</p>
              </div>
            )}
          {error && (
            <div className="error" role="alert">
              <strong>无法完成计算</strong>
              <p>{error}</p>
            </div>
          )}
          {!!optimizationProgress.length && !!busy && (
            <ProgressView history={optimizationProgress} live />
          )}
          {candidate && (
            <div className="candidate">
              <div className="candidate-heading">
                <Sparkles size={17} />
                <strong>
                  {!adoptionBlockers(candidate).length
                    ? "候选方案已就绪"
                    : "候选未满足约束"}
                </strong>
                <span>
                  RMS {fmt(candidate.before_rms_um)} →{" "}
                  {fmt(candidate.after_rms_um)} µm · {candidate.iterations}{" "}
                  次迭代 · {candidate.sampling}×{candidate.sampling} 复核
                </span>
              </div>
              <p>{candidate.note}</p>
              {!!adoptionBlockers(candidate).length && (
                <section className="adoption-blockers" role="alert">
                  <strong>
                    无法采纳：{adoptionBlockers(candidate).length} 项原因
                  </strong>
                  <ul>
                    {adoptionBlockers(candidate).map((reason, i) => (
                      <li key={i}>{reason}</li>
                    ))}
                  </ul>
                  <p>下方默认只显示未通过项。其余项目通过不会抵消这些限制。</p>
                </section>
              )}
              <ConstraintTable checks={candidate.checks} />
              <OptimizationReportView candidate={candidate} />
              <button
                className="primary"
                disabled={!!adoptionBlockers(candidate).length}
                title={
                  adoptionBlockers(candidate).join("；") ||
                  "全部检查通过，采纳此候选"
                }
                onClick={() => {
                  if (adoptionBlockers(candidate).length) return;
                  const next = restoreVersion(project, candidate.project);
                  if (project.workspace?.motion) {
                    ensureWorkspace(next).motion = copy(
                      project.workspace.motion,
                    );
                    synchronizeMotion(project, next);
                  }
                  replace(next);
                }}
              >
                采纳候选
              </button>
              <button
                onClick={() => {
                  try {
                    const c = candidate;
                    edit((p) =>
                      addVersion(
                        p,
                        c.project,
                        `${c.accepted ? "优化候选" : "待改进候选"} ${(project.workspace?.versions.length ?? 0) + 1}`,
                        c.note,
                      ),
                    );
                    setCandidate(c);
                    setTab("versions");
                    setNotice("候选已记录为版本；当前处方未改变。");
                  } catch (e) {
                    setError(explainError(e));
                  }
                }}
              >
                保存候选为版本
              </button>
              <button onClick={() => setCandidate(null)}>放弃</button>
              {candidate.report && (
                <button
                  onClick={() =>
                    void exportReport("optimization-report.json", {
                      format: "OSS optimization 1",
                      candidate,
                      configuration: {
                        fields: candidate.project.fields,
                        wavelengths: candidate.project.wavelengths,
                      },
                      constraints: project.workspace?.constraints,
                    }).catch((e) => setError(explainError(e)))
                  }
                >
                  导出优化报告
                </button>
              )}
            </div>
          )}
          <section className="analysis" id="analysis-workbench">
            {tab === "optimization" && (
              <div className="workflow-panel optimization-workbench">
                <h2>约束优化</h2>
                <p>
                  先记录基准版本，选择变量和边界；候选通过最终复核后再采纳。
                </p>
                <ConstraintEditor
                  value={
                    project.workspace?.constraints ??
                    defaultConstraints(project, result?.paraxial.efl ?? 50)
                  }
                  onChange={(c) =>
                    edit((p) => {
                      ensureWorkspace(p).constraints = c;
                    })
                  }
                />
                <OptimizationEditor
                  resolvedFields={result?.fields.map((f) => f.field)}
                  project={project}
                  value={
                    project.workspace?.optimization ??
                    defaultOptimization(project, selected)
                  }
                  onChange={(o) =>
                    edit((p) => {
                      ensureWorkspace(p).optimization = o;
                    })
                  }
                />
                <p className="hint">
                  固定材料；约束焦距、总长、相交与采样通光。优化结果需要手动采纳。
                </p>
                <button
                  className="focus-button"
                  disabled={
                    !!busy ||
                    !project.surfaces.length ||
                    !!optimizationError(
                      project,
                      project.workspace?.optimization ??
                        defaultOptimization(project, selected),
                    ) ||
                    !(
                      project.workspace?.optimization ??
                      defaultOptimization(project, selected)
                    ).variables.some((v) => !v.locked)
                  }
                  onClick={() => void run("optimize")}
                >
                  <Sparkles size={16} />
                  生成优化候选
                </button>
                <p className="hint">
                  F
                  数根据入瞳计算。修改光阑会改变通光；焦距由镜片形状、间隔与材料共同决定。
                </p>
              </div>
            )}
            {tab === "drawing" && (
              <Suspense
                fallback={<p className="workflow-panel">载入技术图纸工具…</p>}
              >
                <DrawingPanel
                  project={project}
                  onHelp={() => setHelpTopic("drawing")}
                />
              </Suspense>
            )}
            {tab === "geometry" && (
              <GeometryPanel result={geometry} onSelect={setSelected} />
            )}
            {tab === "focus" && (
              <FocusPanel
                project={project}
                edit={edit}
                onCandidate={(c) =>
                  receiveCandidate(c, project, sourceDocumentStamp)
                }
              />
            )}
            {tab === "aperture" && (
              <AperturePanel
                project={project}
                edit={edit}
                onCandidate={(c) =>
                  receiveCandidate(c, project, sourceDocumentStamp)
                }
              />
            )}
            {tab === "goals" && (
              <GoalPanel
                project={project}
                result={result}
                edit={edit}
                disabled={!!busy || !project.surfaces.length}
                optimize={() => void run("optimize")}
              />
            )}
            {tab === "pupil" && (
              <PupilPanel
                project={project}
                onRay={setSelectedRay}
                onSurface={setSelected}
              />
            )}
            {tab === "motion" && <MotionPanel project={project} edit={edit} />}
            {tab === "groups" && (
              <GroupPanel
                project={project}
                edit={edit}
                onSurface={setSelected}
              />
            )}
            {tab === "versions" && (
              <VersionPanel
                project={project}
                edit={edit}
                restore={(p, preset, groups, motion) =>
                  replace(
                    restoreVersion(project, p, preset, groups ?? [], motion),
                  )
                }
              />
            )}
            {tab === "spots" && (
              <>
                <div className="analysis-heading">
                  <div>
                    <h2>把画质看得更清楚</h2>
                    <p>
                      光点越集中，几何成像越清晰。所有波长围绕共同质心显示，保留横向色差。
                    </p>
                  </div>
                  <span className="tag">SPOT DIAGRAM</span>
                </div>
                <Spots result={result} />
              </>
            )}
            {tab === "prescription" && (
              <div className="tablewrap">
                <table>
                  <thead>
                    <tr>
                      {[
                        "表面",
                        "曲率 1/mm",
                        "厚度 mm",
                        "半口径 mm",
                        "材料",
                      ].map((t) => (
                        <th key={t}>{t}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {project.surfaces.map((v, i) => (
                      <tr
                        key={i}
                        onClick={() => setSelected(i)}
                        className={i === selected ? "highlight" : ""}
                      >
                        <td>
                          {i + 1} · {v.name}
                        </td>
                        <td>{fmt(v.curvature, 5)}</td>
                        <td>{fmt(v.thickness, 3)}</td>
                        <td>{fmt(v.semi_aperture, 3)}</td>
                        <td>{project.materials[v.material]?.name}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <p>
                  选中表面后，在右侧参数面板编辑。最后一面的厚度表示到传感器的距离。
                </p>
              </div>
            )}
            {tab === "aberration" && (
              <div className="tablewrap">
                <h3>色差、场曲与像散</h3>
                {result && (
                  <RayDiagnostics result={result} onSelect={setSelected} />
                )}
                <table>
                  <thead>
                    <tr>
                      <th>半视场</th>
                      <th>多色 RMS µm</th>
                      <th>横向色差 µm</th>
                      <th>弧矢焦移 mm</th>
                      <th>子午焦移 mm</th>
                    </tr>
                  </thead>
                  <tbody>
                    {result?.fields.map((v, i) => (
                      <tr key={i}>
                        <td>{fmt((v.field * 180) / Math.PI, 1)}°</td>
                        <td>{fmt(v.polychromatic_rms_um)}</td>
                        <td>{fmt(v.lateral_color_um)}</td>
                        <td>{fmt(v.sagittal_focus_offset_mm, 4)}</td>
                        <td>{fmt(v.tangential_focus_offset_mm, 4)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <p>
                  弧矢 / 子午焦移采用 d
                  线主光线附近的微分光束，相对于当前像面；二者之差反映像散。横向色差为所选波长质心
                  Y 的跨度。
                </p>
                <table>
                  <thead>
                    <tr>
                      <th>波长 nm</th>
                      <th>近轴纵向色差 mm（相对 d 线）</th>
                    </tr>
                  </thead>
                  <tbody>
                    {result?.chromatic_focus.map((v, i) => (
                      <tr key={i}>
                        <td>{fmt(v.wavelength_nm, 2)}</td>
                        <td>{fmt(v.paraxial_focus_offset_mm, 5)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <h3 style={{ marginTop: 20 }}>各波长追迹结果</h3>
                <table>
                  <thead>
                    <tr>
                      <th>视场</th>
                      <th>波长 nm</th>
                      <th>RMS µm</th>
                      <th>质心 Y mm</th>
                      <th>畸变 %</th>
                      <th>遮挡 / 失败</th>
                    </tr>
                  </thead>
                  <tbody>
                    {result?.spots.map((v, i) => (
                      <tr key={i}>
                        <td>{fmt((v.field * 180) / Math.PI, 1)}°</td>
                        <td>{fmt(v.wavelength_nm, 1)}</td>
                        <td>{fmt(v.rms_um)}</td>
                        <td>{fmt(v.centroid?.[1], 4)}</td>
                        <td>{fmt(v.distortion_percent)}</td>
                        <td>
                          {(v.blocked_fraction * 100).toFixed(0)}% / {v.failed}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <p>
                  遮挡比例不是相对照度。畸变采用无限远目标的 f·tan(θ)
                  参考像高；有限物距暂不报告该指标。
                </p>
              </div>
            )}
            {tab === "preview" && project.surfaces.length > 0 && (
              <PreviewPanel
                project={project}
                disabled={!!busy || !project.surfaces.length}
              />
            )}
            {tab === "wave" && project.surfaces.length > 0 && (
              <WavePanel
                project={project}
                disabled={!!busy || !project.surfaces.length}
              />
            )}
          </section>
          {issues.length > 0 && (
            <div className="diagnostics">
              {issues.map((d, i) => (
                <p key={i}>
                  {d.surface === null ? "系统" : `第 ${d.surface + 1} 面`}：
                  {diagnostic(d.message)}
                </p>
              ))}
            </div>
          )}
        </main>
        <aside
          className="right"
          aria-label="参数检查器"
          hidden={workspaceLayout.rightHidden}
        >
          <div className="inspector-head">
            <div className="inspector-heading">
              <Settings2 size={14} />
              <strong>参数设置</strong>
              <span>{s ? `S${selected + 1}` : "空白项目"}</span>
            </div>
            <div className="mode">
              <button
                className={mode === "basic" ? "active" : ""}
                onClick={() => setMode("basic")}
              >
                基础
              </button>
              <button
                className={mode === "pro" ? "active" : ""}
                onClick={() => setMode("pro")}
              >
                专业
              </button>
            </div>
            <nav className="inspector-links" aria-label="设置页面">
              <button
                aria-pressed={inspectorTab === "element"}
                className={inspectorTab === "element" ? "active" : ""}
                onClick={() => setInspectorTab("element")}
              >
                镜片 / 面
              </button>
              <button
                aria-pressed={inspectorTab === "photography"}
                className={inspectorTab === "photography" ? "active" : ""}
                onClick={() => setInspectorTab("photography")}
              >
                摄影与分析
              </button>
            </nav>
          </div>
          <div
            className="inspector-scroll independent-scroll"
            aria-label="参数设置滚动区"
            role="region"
            tabIndex={0}
            ref={rightScroll}
          >
            <div hidden={inspectorTab !== "element"}>
              <MaterialLibrary
                project={project}
                selected={selected}
                edit={edit}
              />
              {mode === "basic" && selectedLens && (
                <LensEditor
                  project={project}
                  lens={selectedLens}
                  edit={edit}
                  select={setSelected}
                />
              )}
              {s && (mode === "pro" || !selectedLens) && (
                <>
                  <div className="inspector-title">
                    <Settings2 size={16} />
                    <h3>{s.name}</h3>
                    <span>S{selected + 1}</span>
                  </div>
                  <label className="text-label">
                    表面名称
                    <input
                      value={s.name}
                      onChange={(e) => update({ name: e.target.value })}
                    />
                  </label>
                  <Num
                    label="曲率"
                    value={s.curvature}
                    unit="1/mm"
                    step={0.001}
                    onChange={(v) => update({ curvature: v })}
                  />
                  <Num
                    label={
                      selected === project.surfaces.length - 1
                        ? "到像面距离"
                        : "到下一面距离"
                    }
                    value={s.thickness}
                    unit="mm"
                    min={0}
                    onChange={(v) => update({ thickness: v })}
                  />
                  <Num
                    label="半口径"
                    value={s.semi_aperture}
                    unit="mm"
                    min={0.01}
                    onChange={(v) => update({ semi_aperture: v })}
                  />
                  <label className="text-label">
                    面后材料
                    <select
                      value={s.material}
                      onChange={(e) =>
                        update({ material: Number(e.target.value) })
                      }
                    >
                      {project.materials.map((m, i) => (
                        <option key={i} value={i}>
                          {m.name}
                        </option>
                      ))}
                    </select>
                  </label>
                  {mode === "pro" && (
                    <MaterialEditor
                      material={project.materials[s.material]}
                      onApply={(m) =>
                        edit((p) => {
                          p.materials[s.material] = m;
                        })
                      }
                    />
                  )}
                  <div className="inline-actions">
                    <button
                      disabled={selected === project.stop}
                      onClick={() =>
                        edit((p) => {
                          p.stop = selected;
                        })
                      }
                    >
                      设为光阑
                    </button>
                    <button
                      title="删除表面"
                      aria-label="删除表面"
                      disabled={project.surfaces.length <= 1}
                      onClick={() =>
                        edit((p) => {
                          p.surfaces.splice(selected, 1);
                          if (p.stop > selected) p.stop--;
                          else if (p.stop === selected)
                            p.stop = Math.min(selected, p.surfaces.length - 1);
                        })
                      }
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                  {mode === "pro" && (
                    <>
                      <Num
                        label="圆锥常数 K"
                        value={s.conic}
                        onChange={(v) => update({ conic: v })}
                      />
                      {s.asphere.map((a, i) => (
                        <Num
                          key={i}
                          label={`A${4 + i * 2}`}
                          value={a}
                          step={1e-6}
                          onChange={(v) => {
                            const a = [...s.asphere] as Surface["asphere"];
                            a[i] = v;
                            update({ asphere: a });
                          }}
                        />
                      ))}
                    </>
                  )}
                </>
              )}
            </div>
            <div hidden={inspectorTab !== "photography"}>
              <div className="section-label">摄影设置</div>
              <button
                className="settings-context-help"
                onClick={() => setHelpTopic("fields")}
              >
                摄影与视场设置帮助
              </button>
              <button
                className="focus-button"
                onClick={() => navigate("goals")}
              >
                设置传感器联动视场
              </button>
              {project.surfaces.length > 0 && (
                <>
                  <Num
                    label="光阑半径"
                    value={project.surfaces[project.stop].semi_aperture}
                    unit="mm"
                    min={0.1}
                    onChange={(v) =>
                      edit((p) => {
                        p.surfaces[p.stop].semi_aperture = v;
                      })
                    }
                  />
                  <input
                    aria-label="拖动光阑半径"
                    type="range"
                    className="aperture-slider"
                    min="0.5"
                    max={Math.max(
                      12,
                      project.surfaces[project.stop].semi_aperture,
                    )}
                    step="0.1"
                    value={project.surfaces[project.stop].semi_aperture}
                    onPointerDown={(e) => {
                      e.currentTarget.setPointerCapture(e.pointerId);
                      beginScrub();
                    }}
                    onPointerUp={endScrub}
                    onPointerCancel={endScrub}
                    onKeyDown={beginScrub}
                    onKeyUp={endScrub}
                    onBlur={endScrub}
                    onChange={(e) => {
                      const v = Number(e.target.value);
                      const p = latestProject.current;
                      if (p) {
                        const q = copy(p);
                        q.surfaces[q.stop].semi_aperture = v;
                        synchronizeMotion(p, q);
                        installProject(q);
                      }
                    }}
                  />
                </>
              )}
              <label className="check">
                <input
                  type="checkbox"
                  checked={project.object_distance === null}
                  onChange={(e) =>
                    edit((p) => {
                      p.object_distance = e.target.checked ? null : 1000;
                    })
                  }
                />
                无限远物体
              </label>
              {project.object_distance !== null && (
                <Num
                  label="物距"
                  value={project.object_distance}
                  min={1.01}
                  unit="mm"
                  onChange={(v) =>
                    edit((p) => {
                      p.object_distance = v;
                    })
                  }
                />
              )}
              <Num
                label="传感器宽度"
                value={project.sensor[0]}
                min={0.1}
                unit="mm"
                onChange={(v) =>
                  edit((p) => {
                    p.sensor[0] = v;
                  })
                }
              />
              <Num
                label="传感器高度"
                value={project.sensor[1]}
                min={0.1}
                unit="mm"
                onChange={(v) =>
                  edit((p) => {
                    p.sensor[1] = v;
                  })
                }
              />
              <button
                className="focus-button"
                disabled={!!busy || !project.surfaces.length}
                onClick={() => void run("autofocus")}
              >
                <Focus size={16} />
                自动对焦 <span>生成候选</span>
              </button>
              <button
                className="focus-button"
                disabled={!project.surfaces.length}
                onClick={() => setTab("focus")}
              >
                打开对焦扫描
              </button>
              <details>
                <summary>分析设置</summary>
                <Num
                  label="手动最大半视场（改为角度模式）"
                  value={
                    (Math.max(
                      ...(
                        result?.fields.map((f) => f.field) ?? project.fields
                      ).map(Math.abs),
                    ) *
                      180) /
                    Math.PI
                  }
                  min={0}
                  max={40.1}
                  unit="°"
                  onChange={(v) =>
                    edit((p) => {
                      const a = (v * Math.PI) / 180;
                      p.fields = [0, a * 0.7, a];
                      delete p.field_spec;
                    })
                  }
                />
                {project.wavelengths.map((w, i) => (
                  <div key={i}>
                    <Num
                      label={`波长 ${i + 1}`}
                      value={w.nm}
                      min={380}
                      max={780}
                      unit="nm"
                      onChange={(v) =>
                        edit((p) => {
                          p.wavelengths[i].nm = v;
                        })
                      }
                    />
                    <Num
                      label={`权重 ${i + 1}`}
                      value={w.weight}
                      min={0.01}
                      onChange={(v) =>
                        edit((p) => {
                          p.wavelengths[i].weight = v;
                        })
                      }
                    />
                  </div>
                ))}
              </details>
            </div>
          </div>
        </aside>
      </div>
      <footer>
        <span className={busy ? "status busy" : "status"}>
          {busy ? (
            <LoaderCircle size={13} className="spin" />
          ) : (
            <span className="live-dot" />
          )}
          {busy ||
            (error
              ? "请修正模型"
              : !project.surfaces.length
                ? "空白项目 · 等待添加镜片"
                : "计算完成")}{" "}
          {busy && <button onClick={cancel}>取消</button>}
        </span>
        <span>
          {notice || "长度 mm · 角度 rad · 波长 nm"}
          {notice && (
            <button aria-label="关闭提示" onClick={() => setNotice("")}>
              <X size={12} />
            </button>
          )}
        </span>
        <button onClick={() => setAnalysisEpoch((v) => v + 1)}>重新分析</button>
      </footer>
      <Suspense fallback={null}>
        {helpTopic && (
          <HelpCenter
            initial={helpTopic}
            close={() => setHelpTopic(null)}
            onNavigate={navigate}
          />
        )}
        {showSearch && (
          <CommandSearch
            close={() => setShowSearch(false)}
            navigate={navigate}
            help={setHelpTopic}
          />
        )}
      </Suspense>
      {showNew && (
        <NewProject
          library={library}
          onCancel={() => setShowNew(false)}
          onCreate={(p) => {
            loadDocument(p);
            setShowNew(false);
          }}
        />
      )}
      {pending && (
        <Modal
          title="保存当前项目的修改？"
          onCancel={() => {
            if (!saving) {
              setPending(false);
              pendingAction.current = null;
            }
          }}
        >
          <p>“{project.name}”有未保存的修改。继续操作前可以保存到项目文件。</p>
          {saveProblem && <p role="alert">{saveProblem}</p>}
          <div className="dialog-actions">
            <button
              disabled={saving}
              onClick={() => {
                setPending(false);
                pendingAction.current = null;
              }}
            >
              取消
            </button>
            <button
              disabled={saving}
              onClick={() => {
                pendingDiscard.current?.();
                continueAction();
              }}
            >
              不保存，继续
            </button>
            <button
              disabled={saving}
              className="primary"
              onClick={() => {
                void saveFile().then((ok) => {
                  if (ok) continueAction();
                });
              }}
            >
              保存后继续
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
