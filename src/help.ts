import { matchesQuery } from "./navigation";
const sources = import.meta.glob("../docs/manual/*.md", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;
const order = [
  "start",
  "workspace",
  "files",
  "editing",
  "mechanics",
  "materials",
  "fields",
  "analysis",
  "pupil",
  "focus",
  "aperture",
  "optimization",
  "groups",
  "motion",
  "study",
  "versions",
  "diffraction",
  "imaging",
  "drawing",
  "troubleshooting",
  "glossary",
  "technical",
];
export const articles = order.map((id) => {
  const body =
    Object.entries(sources).find(([path]) => path.endsWith(`/${id}.md`))?.[1] ??
    "";
  return { id, title: body.split("\n")[0].replace(/^#\s*/, ""), body };
});
export const findArticles = (query: string) =>
  articles.filter((a) => matchesQuery(a.title + "\n" + a.body, query));
