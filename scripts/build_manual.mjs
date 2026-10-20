import { readFileSync, writeFileSync } from "node:fs";
const ids = [
  "start",
  "workspace",
  "files",
  "editing",
  "mechanics",
  "materials",
  "achromat",
  "synthesis",
  "coatings",
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
const docs = ids.map((id) => ({
  id,
  text: readFileSync(`docs/manual/${id}.md`, "utf8"),
}));
const escape = (s) =>
  s
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
writeFileSync(
  "docs/OOS-1.3-使用手册.md",
  "# OOS 1.3 完整使用手册\n\n本手册包含新手教程、全部功能说明和技术文档；软件内 F1 / 帮助入口可全文检索相同内容。\n\n" +
    docs
      .map((d) => d.text.replace(/\]\(([a-z-]+)\.md\)/g, "](manual/$1.md)"))
      .join("\n\n---\n\n"),
);
const inline = (s) =>
  escape(s)
    .replace(/\[([^\]]+)\]\(([a-z-]+)\.md\)/g, '<a href="#$2">$1</a>')
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
const content = docs
  .map(
    (d) =>
      `<article id="${d.id}">` +
      d.text
        .split("\n")
        .map((t) => {
          if (!t.trim() || t.startsWith("```")) return "";
          const h = /^(#{1,3}) (.*)/.exec(t);
          return h
            ? `<h${h[1].length}>${inline(h[2])}</h${h[1].length}>`
            : `<p>${inline(t)}</p>`;
        })
        .join("\n") +
      "</article>",
  )
  .join("\n");
const toc = docs
  .map(
    (d) =>
      `<a href="#${d.id}">${escape(d.text.split("\n")[0].replace(/^# /, ""))}</a>`,
  )
  .join("");
writeFileSync(
  "docs/OOS-1.3-使用手册.html",
  `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>OOS 1.3 完整使用手册</title><style>body{margin:0;font:16px/1.9 "Segoe UI","Microsoft YaHei",sans-serif;color:#23353f;background:#f7f9fb}nav{position:fixed;inset:0 auto 0 0;width:250px;overflow:auto;padding:24px;background:#16232b;box-sizing:border-box}nav a{display:block;color:#d1e6bb;text-decoration:none;padding:7px 0}main{max-width:950px;margin-left:270px;padding:30px}article{padding:15px 10px 35px;border-bottom:1px solid #c6d3da;scroll-margin-top:20px}h1{font-size:27px}h2{font-size:21px;margin-top:35px}a{color:#216b70}code{padding:2px 5px;background:#e6ecef;overflow-wrap:anywhere}p{overflow-wrap:anywhere}input{padding:12px;font:inherit;width:100%;box-sizing:border-box;border:1px solid #738a98;border-radius:4px}label{display:block;position:sticky;top:0;background:#f7f9fb;padding-bottom:15px}.hidden{display:none}@media(max-width:800px){nav{position:static;width:auto}main{margin:0;padding:20px}}@media print{nav,label{display:none}main{margin:0;max-width:none}article{break-before:page}}</style><nav><h2 style="color:white">OOS 1.3 使用手册</h2>${toc}</nav><main><label>全文检索（空格分隔多个关键词）<input id="search" placeholder="例如：角落、MTF、无法采纳"></label><div id="empty" hidden>没有匹配章节，请尝试其他词。</div>${content}</main><script>document.getElementById('search').addEventListener('input',e=>{const q=e.target.value.normalize('NFKC').toLowerCase().trim().split(/\\s+/);let found=0;document.querySelectorAll('article').forEach(a=>{const match=q.every(t=>a.textContent.normalize('NFKC').toLowerCase().includes(t));a.classList.toggle('hidden',!match);if(match)found++});document.getElementById('empty').hidden=found>0});document.querySelectorAll('a[href^="#"]').forEach(a=>a.addEventListener('click',()=>{document.getElementById('search').value='';document.querySelectorAll('article').forEach(t=>t.classList.remove('hidden'));document.getElementById('empty').hidden=true}));</script></html>`,
);
console.log(`Built offline manual: ${docs.length} chapters.`);
