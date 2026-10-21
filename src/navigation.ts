export const sections = [
  {
    id: "design",
    label: "设计",
    hint: "定义目标、搭建镜头并检查几何",
    tabs: [
      ["synthesis", "从指标创建设计", "synthesis"],
      ["goals", "设计目标", "fields"],
      ["coatings", "镀膜与透射", "coatings"],
      ["prescription", "镜头处方", "editing"],
      ["groups", "镜组管理", "groups"],
      ["spatial", "三维追迹与调试", "spatial"],
      ["geometry", "几何检查", "mechanics"],
    ],
  },
  {
    id: "quality",
    label: "画质",
    hint: "先检查通光，再评估几何与衍射画质",
    tabs: [
      ["spots", "点列图", "analysis"],
      ["quality_report", "画质验收报告", "analysis"],
      ["pupil", "瞳面图", "pupil"],
      ["aberration", "像差与通光", "analysis"],
      ["wave", "PSF / MTF", "diffraction"],
      ["preview", "测试图成像", "imaging"],
    ],
  },
  {
    id: "tune",
    label: "调校",
    hint: "保存基准后进行扫描或受约束的局部优化",
    tabs: [
      ["focus", "像面对焦扫描", "focus"],
      ["motion", "变焦与对焦工作台", "study"],
      ["aperture", "光圈扫描", "aperture"],
      ["optimization", "约束优化", "optimization"],
    ],
  },
  {
    id: "deliver",
    label: "交付",
    hint: "比较设计、导出图纸并保存完整项目",
    tabs: [
      ["versions", "版本对比", "versions"],
      ["drawing", "技术图纸 PDF", "drawing"],
    ],
  },
];
export const allTabs = sections.flatMap((s) => s.tabs);
export const sectionFor = (tab: string) =>
  sections.find((s) => s.tabs.some((t) => t[0] === tab)) ?? sections[1];
export const helpFor = (tab: string) =>
  allTabs.find((t) => t[0] === tab)?.[2] ?? "start";
export function matchesQuery(text: string, query: string) {
  const source = text.normalize("NFKC").toLocaleLowerCase();
  return query
    .normalize("NFKC")
    .toLocaleLowerCase()
    .trim()
    .split(/\s+/)
    .every((q) => source.includes(q));
}
