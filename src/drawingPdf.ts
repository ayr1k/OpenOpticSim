import { PDFDocument, PDFPage, rgb } from "pdf-lib";
import fontkit from "@pdf-lib/fontkit";
import {
  drawingGeometry,
  drawingLensGroups,
  drawingStatus,
  fixed,
  type DrawingSnapshot,
} from "./drawing";
import { outerRadius } from "./project";
import { diagnostic } from "./errors";
const W = 841.89,
  H = 595.28,
  M = 42,
  CW = W - 2 * M;
const ink = rgb(0.12, 0.19, 0.23),
  muted = rgb(0.35, 0.42, 0.46),
  accent = rgb(0.12, 0.36, 0.39),
  pale = rgb(0.94, 0.96, 0.97);
export async function createDrawingPdf(
  s: DrawingSnapshot,
  fontBytes: Uint8Array,
): Promise<Uint8Array> {
  const doc = await PDFDocument.create();
  doc.registerFontkit(fontkit);
  const font = await doc.embedFont(fontBytes, { subset: true });
  const glyphs = new Set(font.getCharacterSet());
  const clean = (t: string) =>
    [...String(t)]
      .map((c) => (glyphs.has(c.codePointAt(0)!) ? c : "?"))
      .join("");
  const p = s.project;
  doc.setTitle(`${p.name} - OOS 技术图纸`);
  doc.setAuthor("OpenOpticSim 1.5");
  doc.setSubject("旋转对称顺序透射光学设计记录");
  doc.setCreator("OOS 1.5 / pdf-lib");
  doc.setCreationDate(new Date(s.createdAt));
  let page!: PDFPage;
  let y = 0,
    section = "";
  function text(
    t: string,
    x: number,
    top: number,
    size = 10,
    color = ink,
    dest = page,
  ) {
    dest.drawText(clean(t), { x, y: H - top - size, size, font, color });
  }
  function line(
    x1: number,
    t1: number,
    x2: number,
    t2: number,
    color = muted,
    width = 0.55,
    dest = page,
  ) {
    dest.drawLine({
      start: { x: x1, y: H - t1 },
      end: { x: x2, y: H - t2 },
      color,
      thickness: width,
    });
  }
  function newPage(title: string) {
    section = title.replace(/（续）$/, "");
    page = doc.addPage([W, H]);
    text("OpenOpticSim", M, 23, 11, accent);
    text("1.5  /  OPTICAL DESIGN RECORD", W - 265, 24, 8, muted);
    line(M, 45, W - M, 45);
    text(title, M, 59, 18);
    y = 94;
  }
  function ensure(height: number) {
    if (y + height > H - 53) newPage(section + "（续）");
  }
  function wrap(value: string, width: number, size = 10) {
    const out: string[] = [];
    for (const paragraph of String(value).split("\n")) {
      let row = "";
      for (const c of clean(paragraph)) {
        if (row && font.widthOfTextAtSize(row + c, size) > width) {
          out.push(row);
          row = "";
        }
        row += c;
      }
      out.push(row);
    }
    return out;
  }
  function paragraph(value: string, size = 10, color = ink) {
    for (const t of wrap(value, CW, size)) {
      ensure(size + 7);
      text(t, M, y, size, color);
      y += size + 7;
    }
    y += 6;
  }
  function heading(value: string) {
    ensure(45);
    text(value, M, y, 13, accent);
    y += 26;
  }
  function table(
    headers: string[],
    rows: string[][],
    widths: number[],
    size = 8.7,
  ) {
    function row(cells: string[], head = false) {
      const lines = cells.map((v, i) => wrap(v, widths[i] - 12, size));
      let offset = 0;
      const count = Math.max(...lines.map((a) => a.length));
      while (offset < count) {
        const remaining = (count - offset) * (size + 5) + 8;
        if (y + Math.min(remaining, 2 * (size + 5) + 8) > H - 53) {
          newPage(section + "（续）");
          if (!head) row(headers, true);
        }
        const take = Math.max(
          1,
          Math.min(count - offset, Math.floor((H - 53 - y - 8) / (size + 5))),
        );
        const height = take * (size + 5) + 8;
        if (head)
          page.drawRectangle({
            x: M,
            y: H - y - height,
            width: CW,
            height,
            color: pale,
          });
        let x = M;
        lines.forEach((ls, i) => {
          ls.slice(offset, offset + take).forEach((t, j) =>
            text(t, x + 6, y + 4 + j * (size + 5), size, head ? accent : ink),
          );
          x += widths[i];
        });
        line(M, y + height, W - M, y + height, rgb(0.8, 0.85, 0.87), 0.4);
        y += height;
        offset += take;
        if (offset < count) {
          newPage(section + "（续）");
          if (!head) row(headers, true);
        }
      }
    }
    row(headers, true);
    rows.forEach((r) => row(r));
    y += 14;
  }
  function dimension(
    x1: number,
    t1: number,
    x2: number,
    t2: number,
    label: string,
  ) {
    line(x1, t1, x2, t2, muted, 0.5);
    const horiz = Math.abs(t1 - t2) < 1;
    if (horiz) {
      line(x1, t1 - 4, x1, t1 + 4);
      line(x2, t2 - 4, x2, t2 + 4);
      text(
        label,
        (x1 + x2) / 2 - font.widthOfTextAtSize(clean(label), 8) / 2,
        t1 + 5,
        8,
      );
    } else {
      line(x1 - 4, t1, x1 + 4, t1);
      line(x2 - 4, t2, x2 + 4, t2);
      text(label, x1 + 6, (t1 + t2) / 2, 8);
    }
  }
  function figure(
    x: number,
    top: number,
    width: number,
    height: number,
    start = 0,
    end = p.surfaces.length - 1,
    overall = false,
  ) {
    const g = drawingGeometry(p, start, end),
      padX = 45,
      padY = 34;
    const scale = Math.min(
      (width - 2 * padX) / Math.max(1, g.maxZ - g.minZ),
      (height - 2 * padY) / (2 * g.radius),
    );
    const cx = x + width / 2,
      cy = top + height / 2 - 5,
      mz = (g.maxZ + g.minZ) / 2;
    const sx = (z: number) => cx + (z - mz) * scale,
      sy = (r: number) => cy - r * scale;
    line(x + 8, cy, x + width - 10, cy, rgb(0.67, 0.73, 0.76), 0.45);
    const path = (points: number[][], close = false, fill = false) => {
      if (!points.length) return;
      const d =
        points
          .map((q, i) => `${i ? "L" : "M"}${sx(q[0])} ${sy(q[1])}`)
          .join(" ") + (close ? " Z" : "");
      page.drawSvgPath(d, {
        x: 0,
        y: H,
        borderColor: accent,
        borderWidth: 0.8,
        ...(fill ? { color: rgb(0.87, 0.94, 0.96) } : {}),
      });
    };
    for (let i = start; i < end; i++)
      if (p.surfaces[i].material !== 0) {
        const a = g.paths.find((a) => a.index === i),
          b = g.paths.find((a) => a.index === i + 1);
        if (a && b) path([...a.points, ...[...b.points].reverse()], true, true);
      }
    g.paths.forEach((a) => path(a.points));
    if (overall) {
      const z = g.imageZ,
        r = Math.min(g.radius, Math.hypot(...p.sensor) / 2);
      line(sx(z), sy(-r), sx(z), sy(r), rgb(0.62, 0.38, 0.13), 1.5);
      text("像面", sx(z) - 12, sy(r) - 17, 8);
      dimension(
        sx(0),
        top + height - 18,
        sx(z),
        top + height - 18,
        `首顶点至像面 ${fixed(z, 4)} mm`,
      );
      const labels = [0, p.stop, p.surfaces.length - 1].filter(
        (v, i, a) => a.indexOf(v) === i,
      );
      labels.forEach((i, j) => {
        const tx = sx(g.vertices[i]),
          ty = top + 8 + j * 13;
        text(
          `S${i + 1}${i === p.stop ? " · 光阑" : ""}`,
          Math.min(x + width - 70, Math.max(x + 8, tx)),
          ty,
          8,
        );
        line(tx, ty + 11, tx, sy(g.radius) + 5, muted, 0.4);
      });
    } else {
      dimension(
        sx(g.vertices[start]),
        top + height - 18,
        sx(g.vertices[end]),
        top + height - 18,
        `轴上长度 ${fixed(g.vertices[end] - g.vertices[start], 4)} mm`,
      );
      dimension(
        x + width - 43,
        sy(g.radius),
        x + width - 43,
        sy(-g.radius),
        `Ø${fixed(g.radius * 2, 3)}`,
      );
      g.paths.forEach((a) =>
        text(
          `S${a.index + 1}`,
          sx(g.vertices[a.index]) - 8,
          top + 8 + ((a.index - start) % 2) * 13,
          8,
        ),
      );
    }
    text(
      `等比例；纸面缩放 ${fixed((scale * 25.4) / 72, 3)}:1`,
      x + 8,
      top + height - 1,
      7,
      muted,
    );
    if (g.invalid.length)
      text(
        `未绘制定义域无效面：${g.invalid.map((i) => `S${i + 1}`).join(", ")}`,
        x + 8,
        top + height - 14,
        8,
        rgb(0.7, 0.18, 0.12),
      );
  }
  newPage("镜头技术图纸 / 设计总览");
  paragraph(p.name, 19, accent);
  paragraph(p.description || "无项目描述", 10, muted);
  paragraph(
    `${drawingStatus(s)}。本图纸记录名义设计；未定义加工公差、倒角、表面质量、镀膜、装配基准与机械强度，不能直接作为加工验收依据。`,
    9,
    muted,
  );
  table(
    ["记录项目", "数值 / 状态", "记录项目", "数值 / 状态"],
    [
      [
        "有效焦距 EFL",
        `${fixed(s.analysis?.paraxial.efl)} mm`,
        "后焦距 BFL",
        `${fixed(s.analysis?.paraxial.bfl)} mm`,
      ],
      [
        "近轴 F 数",
        fixed(s.analysis?.paraxial.f_number),
        "传感器",
        `${p.sensor.join(" × ")} mm`,
      ],
      [
        "物距",
        p.object_distance === null
          ? "无限远"
          : `${fixed(p.object_distance)} mm`,
        "表面 / 实体镜片组",
        `${p.surfaces.length} / ${drawingLensGroups(p).length}`,
      ],
      [
        "生成时间",
        s.createdAt,
        "光阑",
        p.surfaces.length
          ? `S${p.stop + 1}；Ø ${fixed(p.surfaces[p.stop].semi_aperture * 2)} mm`
          : "无",
      ],
    ],
    [95, 283.945, 95, 283.945],
  );
  paragraph(`源项目 SHA-256：${s.sourceHash}`, 8, muted);
  paragraph(
    "完整原生项目作为 PDF 附件嵌入，包含未经表格舍入的数值、版本与扫描源记录。提取 project.oos.json 后可在 OOS 中重新打开；普通 PDF 阅读器可能不显示附件入口。",
    9,
    muted,
  );
  if (p.workspace?.motion) {
    const m = p.workspace.motion;
    newPage("镜组运动 / 多配置记录");
    paragraph(
      `当前处方配置：${m.configurations.find((c) => c.id === m.active_id)?.name ?? "未标记 / 已编辑"}。固定传感器参考坐标：${fixed(m.reference.surfaces.reduce((a, s) => a + s.thickness, 0))} mm。`,
    );
    paragraph(
      `最小空气间隙 ${fixed(m.min_air_mm)} mm；RMS 验收上限 ${fixed(m.max_rms_um)} μm；采样 ${m.sampling}×${m.sampling}；目标 ${m.criterion}。图纸分析只属于当前处方，其他配置需单独验证。`,
    );
    table(
      ["镜组", "范围", "行程 mm", "锁定"],
      m.groups.map((g) => [
        g.name,
        `S${g.start + 1}–S${g.end + 1}`,
        `${fixed(g.min_mm)} ~ ${fixed(g.max_mm)}`,
        g.locked ? "是" : "否",
      ]),
      [220, 110, 300, 127.89],
    );
    table(
      ["配置", "参考物距 mm", "镜组位移 mm", "目标 EFL / F 数"],
      m.configurations.map((c) => [
        c.name,
        c.object_distance === null ? "无限远" : fixed(c.object_distance),
        c.positions_mm.map((v) => fixed(v)).join(" / "),
        `${fixed(c.target_efl_mm)} / ${c.target_f_number === null ? "实体光阑" : fixed(c.target_f_number)}`,
      ]),
      [150, 140, 300, 167.89],
    );
  }
  newPage("总装名义剖面 / 轴向尺寸");
  if (p.surfaces.length)
    figure(M, 100, CW, 310, 0, p.surfaces.length - 1, true);
  y = 440;
  paragraph(
    "光线沿 +Z（向右）传播；子午剖面为旋转对称实体。蓝色为光学材料，棕色为传感器。全部尺寸以参数表为准，禁止从打印后的图形量取尺寸。",
    9,
    muted,
  );
  paragraph(
    "总图只标注首面、光阑和末面，密集表面的形状、面号与尺寸见后续分图及完整处方。机械环带保留方边/随曲面轮廓；有效光学口径与机械外径分别列出。",
    9,
    muted,
  );
  const ls = drawingLensGroups(p);
  for (let a = 0; a < ls.length; a += 4) {
    newPage("逐镜片 / 胶合组名义剖面");
    ls.slice(a, a + 4).forEach((lens, j) => {
      const x = M + (j % 2) * (CW / 2 + 7),
        top = 99 + Math.floor(j / 2) * 212,
        w = CW / 2 - 12;
      const fit = (value: string, size: number) => {
        let t = clean(value);
        while (t && font.widthOfTextAtSize(t, size) > w - 8) t = t.slice(0, -1);
        return t === clean(value) ? t : t.slice(0, -1) + "…";
      };
      text(
        fit(
          `${a + j + 1}. ${lens.name}  /  S${lens.start + 1}-S${lens.end + 1}`,
          10,
        ),
        x,
        top,
        10,
        accent,
      );
      figure(x, top + 22, w, 157, lens.start, lens.end);
      text(
        fit(
          `材料：${[...new Set(p.surfaces.slice(lens.start, lens.end).map((v) => p.materials[v.material]?.name))].join(" / ")}`,
          8,
        ),
        x,
        top + 190,
        8,
        muted,
      );
    });
  }
  newPage("完整表面处方 / 顶点坐标");
  paragraph(
    "长度 mm，曲率 1/mm。正 R 表示曲率中心在顶点 +Z 侧；面后材料决定下一段介质，材料 0 为空气。最后一面的厚度表示到传感器的距离。",
    9,
    muted,
  );
  const zs = drawingGeometry(p).vertices;
  table(
    [
      "面号 / 名称",
      "顶点 Z",
      "半径 R / 曲率 c",
      "到下一面 / 像面",
      "有效 Ø / 机械 Ø",
      "面后材料 / 环带",
    ],
    p.surfaces.map((v, i) => [
      `S${i + 1}${i === p.stop ? " [光阑]" : ""}\n${v.name}`,
      fixed(zs[i]),
      `${v.curvature === 0 ? "平面 / ∞" : fixed(1 / v.curvature)}\nc = ${fixed(v.curvature, 12)}`,
      fixed(v.thickness),
      `${fixed(2 * v.semi_aperture)}\n${fixed(2 * outerRadius(v))}`,
      `${v.material} · ${p.materials[v.material]?.name}\n${!v.mechanical ? "无独立环带" : v.mechanical.shape === "flat" ? "方边环带" : "随曲面环带"}`,
    ]),
    [128, 91, 151, 104, 119, 164.89],
  );
  newPage("圆锥与偶次非球面系数");
  paragraph(
    "z(r) = c r² / (1 + sqrt(1 - (1+K)c²r²)) + A4 r⁴ + A6 r⁶ + A8 r⁸ + A10 r¹⁰。r、z 为 mm；A4/A6/A8/A10 的单位分别为 mm^-3 / mm^-5 / mm^-7 / mm^-9。零值表示该项未施加。",
    9,
    muted,
  );
  table(
    ["面号", "圆锥常数 K", "A4", "A6", "A8", "A10"],
    p.surfaces.map((v, i) => [
      `S${i + 1}`,
      String(v.conic),
      ...v.asphere.map((a) => a.toExponential(12)),
    ]),
    [57, 105, 149, 149, 149, 148.89],
    8.2,
  );
  if (p.surfaces.some((s) => s.coating?.length)) {
    newPage("名义镀膜层参数");
    paragraph(
      "顺序从物方到像方。常数折射率、无吸收的教学介质模型；不是加工工艺规范。光学分析未将膜层相位加入 PSF/MTF。",
      9,
      muted,
    );
    table(
      ["面号", "层号", "名称", "折射率 n", "物理厚度 nm"],
      p.surfaces.flatMap((s, i) =>
        (s.coating ?? []).map((l, j) => [
          `S${i + 1}`,
          String(j + 1),
          l.name,
          String(l.index),
          String(l.thickness_nm),
        ]),
      ),
      [60, 60, 337.89, 140, 160],
    );
  }
  newPage("材料快照 / 色散与有效范围");
  paragraph(
    "Sellmeier：n² = 1 + Σ Bi λ² / (λ² - Ci)，λ 使用 µm，Ci 单位 µm²。常数材料使用 n；Sellmeier 材料的 n 字段为保存的常数回退值，实际追迹使用系数。",
    9,
    muted,
  );
  p.materials.forEach((m, i) => {
    ensure(115);
    heading(`${i}. ${m.name}`);
    paragraph(
      `模型：${m.sellmeier ? "Sellmeier" : "常数折射率"}；n = ${m.n}；有效波段 ${m.range_nm.join(" - ")} nm`,
      9,
    );
    if (m.sellmeier) {
      paragraph(
        `B = [${m.sellmeier[0].map((v) => v.toPrecision(16)).join(", ")}]`,
        9,
      );
      paragraph(
        `C = [${m.sellmeier[1].map((v) => v.toPrecision(16)).join(", ")}] µm²`,
        9,
      );
    }
    paragraph(`来源：${m.source || "未注明"}`, 9, muted);
  });
  newPage("视场 / 光谱 / 计算条件");
  paragraph(
    `视场定义：${p.field_spec?.mode === "sensor" ? "跟随传感器（中心、两边中点、角落）" : p.field_spec?.mode === "image_heights" ? "目标像高（mm）" : "物方角度（rad）"}；像面 Z = ${fixed(drawingGeometry(p).imageZ)} mm；本次没有重新对焦；几何采样 ${s.sampling} × ${s.sampling}。`,
    10,
  );
  table(
    ["检查位置", "目标像高 mm", "实际物方角度 °", "实际主光线像高 mm", "状态"],
    s.fields
      ? s.fields.fields.map((f) => [
          f.label,
          fixed(f.target_height_mm),
          fixed(f.angle_rad == null ? null : (f.angle_rad * 180) / Math.PI),
          fixed(f.actual_height_mm),
          f.diagnostic ? diagnostic(f.diagnostic) : f.chief_status,
        ])
      : p.fields.map((f, i) => [
          `${i + 1}`,
          "未求解",
          fixed((f * 180) / Math.PI),
          "未求解",
          "缓存角度，未验证",
        ]),
    [125, 133, 158, 173, 168.89],
  );
  table(
    ["波长 nm", "相对权重（按总和归一化）"],
    p.wavelengths.map((w) => [String(w.nm), String(w.weight)]),
    [200, 557.89],
  );
  if (s.analysis?.coverage) {
    const c = s.analysis.coverage;
    heading("传感器覆盖（不等于像差达标）");
    paragraph(
      `半对角线 ${fixed(c.half_diagonal_mm)} mm；最大已测试主光线像高 ${fixed(c.max_tested_height_mm)} mm；参考波长 ${c.reference_wavelength_nm} nm。`,
      9,
    );
    const labels: Record<string, string> = {
      untested: "未验证",
      unresolved: "求解未完成",
      clipped: "存在物理遮挡",
      sampled_clear: "声明采样通过",
    };
    table(
      ["位置", "像高 mm", "几何状态"],
      c.checkpoints.map((v) => [
        v.label,
        fixed(v.height_mm),
        labels[v.status] ?? v.status,
      ]),
      [200, 200, 357.89],
    );
    c.warnings.forEach((v) => paragraph(diagnostic(v), 9, muted));
  }
  newPage("几何画质与通光 / 当前共同像面");
  if (s.analysis) {
    const a = s.analysis;
    table(
      ["半视场 °", "多色 RMS µm", "横向色差 µm", "弧矢焦移 mm", "子午焦移 mm"],
      a.fields.map((f) => [
        fixed((f.field * 180) / Math.PI),
        fixed(f.polychromatic_rms_um),
        fixed(f.lateral_color_um),
        fixed(f.sagittal_focus_offset_mm),
        fixed(f.tangential_focus_offset_mm),
      ]),
      [150, 151, 151, 151, 154.89],
    );
    table(
      ["视场 ° / 波长 nm", "RMS µm", "质心 X / Y mm", "畸变 %", "失败 / 总数"],
      a.spots.map((v) => [
        `${fixed((v.field * 180) / Math.PI, 4)} / ${v.wavelength_nm}`,
        fixed(v.rms_um),
        v.centroid?.map((q) => fixed(q)).join(" / ") ?? "未定义",
        fixed(v.distortion_percent),
        `${v.failed} / ${v.sampled}`,
      ]),
      [165, 115, 235, 115, 127.89],
    );
    paragraph(
      `入瞳位置 / 半径：${fixed(a.paraxial.entrance_pupil_z)} / ${fixed(a.paraxial.entrance_pupil_radius)} mm；出瞳位置 / 半径：${fixed(a.paraxial.exit_pupil_z)} / ${fixed(a.paraxial.exit_pupil_radius)} mm。`,
      9,
    );
    table(
      ["波长 nm", "近轴纵向色差（相对 d 线）mm"],
      a.chromatic_focus.map((c) => [
        String(c.wavelength_nm),
        fixed(c.paraxial_focus_offset_mm),
      ]),
      [200, 557.89],
    );
    paragraph(
      "失败样本不作为零误差；RMS 只来自有效光线。几何通过比例不是相对照度。有限物距不报告此处采用的无限远畸变参考值。",
      9,
      muted,
    );
  } else paragraph("几何分析未完成；请查看诊断。未以旧结果替代当前处方。", 10);
  newPage("几何分离 / 机械检查");
  if (s.geometry)
    table(
      [
        "区间",
        "介质",
        "轴上厚度 mm",
        "边缘 / 采样最薄 mm",
        "检查半径 mm",
        "连续分离",
      ],
      s.geometry.spans.map((v) => [
        `S${v.front + 1} - S${v.back + 1}`,
        v.glass ? "实体" : "空气",
        fixed(v.center_mm),
        `${fixed(v.edge_mm)} / ${fixed(v.sampled_min_mm)}`,
        fixed(v.radius_mm),
        v.certified ? "确认" : "未确认",
      ]),
      [110, 70, 125, 222, 120, 110.89],
    );
  paragraph(
    "最薄实体/空气间隔的表格数值是采样估计，连续分离状态来自引擎区间检查。未定义机械安装座、装配偏心、公差链或强度。",
    9,
    muted,
  );
  [
    ...s.errors,
    ...(s.geometry?.diagnostics ?? []).map(
      (d) =>
        `${d.surface === null ? "系统" : `S${d.surface + 1}`}：${diagnostic(d.message)}`,
    ),
    ...(s.analysis?.diagnostics ?? []).map(
      (d) =>
        `${d.surface === null ? "系统" : `S${d.surface + 1}`}：${diagnostic(d.message)}`,
    ),
  ].forEach((v) => paragraph(v, 9, rgb(0.65, 0.19, 0.12)));
  if (
    !s.errors.length &&
    !s.geometry?.diagnostics.length &&
    !s.analysis?.diagnostics.length
  )
    paragraph("本次计算没有返回诊断。其含义仅限所声明的模型和采样。", 9);
  if (s.waves.length) {
    for (const w of s.waves) {
      newPage(`衍射 PSF / MTF · 视场 ${w.index + 1}`);
      if (!w.result) {
        paragraph(`未完成：${w.error ?? "未计算"}`);
        continue;
      }
      const a = w.result;
      paragraph(
        `模型：${a.method === "sphere_kirchhoff" ? "直接 Kirchhoff / 平面像面" : "快速小视场"}；多色；物方半视场 ${fixed((a.field * 180) / Math.PI)}°；像面采样 ${fixed(a.pixel_um)} µm / px；频率步长 ${fixed(a.frequency_step_lp_mm)} lp/mm。`,
        9,
      );
      paragraph(
        `收敛：${a.quality.converged ? "通过" : "未通过"}；瞳采样 ${a.quality.previous_sampling} → ${a.quality.pupil_sampling}；MTF 最大变化 ${a.quality.mtf_max_change}；PSF L1 ${a.quality.psf_l1_change}；边缘能量 ${a.quality.edge_energy}。`,
        9,
      );
      const tx = M + 20,
        ty = y + 25,
        pw = 440,
        ph = 190,
        limit = Math.min(
          100,
          (a.mtf_sagittal.length - 1) * a.frequency_step_lp_mm,
        );
      for (let j = 0; j <= 4; j++) {
        const yy = ty + ph - (j * ph) / 4;
        line(tx, yy, tx + pw, yy, rgb(0.82, 0.85, 0.87), 0.4);
        text(String(j / 4), tx - 23, yy - 5, 8);
        text(fixed((limit * j) / 4, 0), tx + (pw * j) / 4 - 5, ty + ph + 8, 8);
      }
      [a.mtf_sagittal, a.mtf_tangential].forEach((v, j) => {
        const color = j ? rgb(0.7, 0.32, 0.12) : accent;
        let prev: number[] | null = null;
        v.forEach((m, k) => {
          const f = k * a.frequency_step_lp_mm;
          if (f > limit) return;
          const now = [tx + (pw * f) / limit, ty + ph * (1 - m)];
          if (prev) line(prev[0], prev[1], now[0], now[1], color, 1.2);
          prev = now;
        });
      });
      text("弧矢 S（蓝绿） / 子午 T（棕） · lp/mm", tx, ty + ph + 27, 9);
      // Grayscale PSF is embedded as an image; all mechanical drawings remain vector.
      const max = a.psf.reduce((m, v) => Math.max(m, v), 0),
        data = new Uint8Array(a.psf.length * 3);
      a.psf.forEach((v, i) => {
        const t = Math.round(
          (255 * Math.log1p((1000 * v) / max)) / Math.log(1001),
        );
        data.set([255 - t, 255 - t, 255 - t], i * 3);
      });
      const image = doc.context.register(
        doc.context.flateStream(data, {
          Type: "XObject",
          Subtype: "Image",
          Width: a.size,
          Height: a.size,
          ColorSpace: "DeviceRGB",
          BitsPerComponent: 8,
        }),
      );
      const name = page.node.newXObject("PSF", image);
      const {
        pushGraphicsState,
        popGraphicsState,
        concatTransformationMatrix,
        drawObject,
      } = await import("pdf-lib");
      page.pushOperators(
        pushGraphicsState(),
        concatTransformationMatrix(190, 0, 0, 190, M + CW - 190, H - ty - 190),
        drawObject(name),
        popGraphicsState(),
      );
      text(
        `PSF 对数显示 / ${fixed(a.size * a.pixel_um, 2)} µm`,
        M + CW - 190,
        ty + 200,
        8,
      );
      y = ty + ph + 54;
      paragraph(a.warning, 8, muted);
      table(
        [
          "波长 nm",
          "权重",
          "波前 RMS / 波",
          "瞳采样 / 遮挡",
          "最大相位步 / 波",
        ],
        a.bands.map((v) => [
          String(v.wavelength_nm),
          String(v.weight),
          fixed(v.rms_waves),
          `${v.pupil_samples} / ${v.blocked_samples}`,
          fixed(v.max_phase_step_waves),
        ]),
        [135, 110, 160, 170, 182.89],
      );
    }
  } else {
    newPage("衍射分析状态");
    paragraph(
      "本次未请求 PSF / MTF；几何分析通过不代表衍射画质已验证。可在导出选项中勾选逐视场多色 PSF / MTF。",
      11,
    );
  }
  newPage("设计设置 / 镜组 / 版本与历史记录");
  paragraph(
    "以下是工作区配置和历史摘要；保存的扫描来自其各自源处方，不作为本次图纸的现时验证结果。完整版本处方、扫描源与全部配置保留于 PDF 的原生项目附件。",
    9,
    muted,
  );
  table(
    ["镜组名称", "表面范围"],
    (p.workspace?.groups ?? []).map((g) => [
      g.name,
      `S${g.start + 1} - S${g.end + 1}`,
    ]),
    [400, 357.89],
  );
  table(
    ["版本名称", "保存时间", "备注"],
    (p.workspace?.versions ?? []).map((v) => [v.name, v.created_at, v.note]),
    [200, 200, 357.89],
  );
  const config = {
    constraints: p.workspace?.constraints ?? null,
    optimization: p.workspace?.optimization ?? null,
    focus: p.workspace?.focus ?? null,
    aperture: p.workspace?.aperture ?? null,
    stored_aperture_scan: p.workspace?.aperture_scan
      ? {
          options: p.workspace.aperture_scan.options,
          points: p.workspace.aperture_scan.points.map((v) => ({
            f_number: v.actual_f_number,
            image_distance_mm: v.image_distance_mm,
            rms_um: v.score_um,
            failed: v.failed,
            sampled: v.sampled,
          })),
        }
      : null,
  };
  heading("配置精确记录（字段与原生格式一致）");
  paragraph(JSON.stringify(config, null, 2), 8);
  newPage("符号 / 范围 / 来源");
  paragraph(
    "本版本为旋转对称、顺序透射模型。无偏心倾斜、自由曲面、偏振、镀膜干涉、非顺序鬼影或真实三维摄影场景渲染。几何测试与衍射收敛只证明相应配置和公开算例，不保证任意系统的实际制造性能。",
    10,
  );
  paragraph(
    "R：曲率半径；c=1/R；K：圆锥常数；A4-A10：偶次非球面系数；Ø：直径；Z：第一顶点为零的轴向坐标；EFL：有效焦距；BFL：末面顶点到无限远近轴焦点；F 数：由近轴入瞳计算；PSF：点扩散函数；MTF：调制传递函数；RMS：关于质心的均方根光斑半径。",
    10,
  );
  paragraph(
    "数据来源：本次导出冻结的用户项目及 OOS 本地计算。没有从图像猜测材料或补全未指定公差。来源哈希由 UTF-8 原生项目 JSON 字节计算；所有显示舍入值的精确原值都在附件中。",
    10,
  );
  paragraph(
    "生成工具：OpenOpticSim 1.5（Apache-2.0）；PDF-LIB / fontkit（MIT）；中文字体 Noto Sans SC（SIL OFL 1.1）。字体来源与许可随安装包分发。",
    9,
    muted,
  );
  await doc.attach(
    new TextEncoder().encode(JSON.stringify(p, null, 2)),
    "project.oos.json",
    {
      mimeType: "application/json",
      description: "完整 OOS 源项目，含精确处方与工作区",
      creationDate: new Date(s.createdAt),
    },
  );
  await doc.attach(
    new TextEncoder().encode(
      JSON.stringify(
        {
          createdAt: s.createdAt,
          sourceHash: s.sourceHash,
          sampling: s.sampling,
          fields: s.fields,
          analysis: s.analysis
            ? {
                ...s.analysis,
                rays: [],
                spots: s.analysis.spots.map((a) => ({ ...a, points: [] })),
              }
            : null,
          geometry: s.geometry,
          waves: s.waves.map((w) => ({
            index: w.index,
            error: w.error,
            result: w.result ? { ...w.result, psf: [] } : null,
          })),
          errors: s.errors,
        },
        null,
        2,
      ),
    ),
    "analysis.json",
    {
      mimeType: "application/json",
      description: "本次计算条件与结果摘要；大型采样数组未附入",
    },
  );
  const pages = doc.getPages();
  pages.forEach((a, i) => {
    line(M, H - 35, W - M, H - 35, muted, 0.4, a);
    let name = clean(p.name);
    while (font.widthOfTextAtSize(name, 7) > 450) name = name.slice(0, -1);
    text(`${name}  |  OOS 1.5  |  名义设计 / mm`, M, H - 26, 7, muted, a);
    text(`${i + 1} / ${pages.length}`, W - M - 40, H - 26, 8, muted, a);
  });
  return doc.save();
}
