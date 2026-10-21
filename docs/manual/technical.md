# 技术文档：模型、算法、接口与验证

## 架构与边界

React / TypeScript 提供编辑和图表，Tauri 2 提供 Windows 文件、任务和安装外壳，自研 Rust CPU f64 核心提供顺序追迹、分析与优化。CLI 和桌面共用核心。PDF 在独立浏览器工作线程使用 pdf-lib/fontkit 和内置中文字体生成；源项目与分析条件冻结后传入，不修改光学核心。

2.0 将核心独立为 OOS-Wollaston v1。既有画质和设计模型仍为旋转对称顺序系统，镀膜直透分析保留。新增三维实验支持表面/镜组偏心倾斜、斜光线和倾斜传感器的顺序几何追迹，详见 [三维追迹](spatial.md)。三维实验不参与同轴优化、PSF/MTF 或 PDF 名义图纸。没有自由曲面、非顺序反射分支、多次鬼影、完整偏振追迹和真实三维场景渲染。

## 坐标、单位和材料

内部 mm、rad、nm；+Z 从物到像，第一面顶点为零；场角位于 Y/Z 面。表面材料是面后的介质，入射介质固定 n=1。有限物点为 (0,-d tan θ,-d)。曲面矢高：

z(r) = c r²/(1+sqrt(1-(1+K)c²r²)) + A4 r⁴ + A6 r⁶ + A8 r⁸ + A10 r¹⁰。

Sellmeier n²=1+Σ Bi λ²/(λ²-Ci)，λ 为 µm。材料有效波段在追迹前检查。原生 JSON 使用 round-trip 浮点解析，v3 的 field_spec 才是像高视场定义，fields 为初始角度缓存。

## 近轴、求交和瞄准

近轴状态为 [高度,n×倾角]，通过折射/传播矩阵计算光焦度、BFL 和瞳共轭。精确追迹以矢高方程求交，Newton 迭代带步长和次数限制，使用向量 Snell 折射，累积材料光程。分别报告未命中、口径遮挡、全反射、定义域与数值不收敛。

主光线采用近轴初值、阻尼 Newton 与视场延拓；目标瞳点围绕主光线逐步延拓，检查相对 Jacobian。瞄准试算忽略上游口径，找到解后重新检查真实全部口径。该方法不穷举折叠或断裂瞳的所有分支。

目标像高沿连接轴上主光线的分支求角度，几何输入界限为 ±0.7 rad。公开验证覆盖特定 32°/40° 原创处方，不保证任意镜头可解。

## 几何指标与优化

圆盘内规则网格采样，单色 RMS 以自身质心，多色以共同质心。失败不是零误差。畸变仅无限远采用 f tan θ 基准；色差和微分场曲定义见相关帮助。

局部优化为有限差分 Jacobian 的阻尼最小二乘，变量投影到用户边界。画质权重、焦距与尺寸约束归一化；最终使用 33×33 通光和连续几何复核。允许分阶段扩大视场与各设计分别对焦，最终实际处方重新计算约束。它不是全局优化。

## 衍射与局部成像

快速模型在规则参考球瞳网格使用局部传播展开与 FFT，有独立视场/方向限制。直接法在最终平面像面直接求和球面传播距离，含实际瞳映射面积、光线管振幅和倾斜因子。多波长按指定权重非相干叠加，计算窗口内归一化；未包括界面 Fresnel、镀膜或传感器响应。

收敛门槛和有限窗口限制见[PSF / MTF](diffraction.md)。成像为局部周期目标与 PSF 卷积，不是完整空间变化渲染。

## JSON 请求接口

CLI：oos-cli --request，从 stdin 读一个 JSON，正常 stdout 输出 JSON，错误 stderr 返回非零。--request-progress 输出 NDJSON 进度，最后一行 result。请求中的 project 是完整光学处方对象，不是路径，工作区由 UI 在调用前分离。

```json
{"operation":"analyze","project":{},"sampling":25}
```

上例 project 只是结构占位，必须替换为有效项目。常用操作包括 validate、geometry、field_plan、analyze、trace、autofocus、focus_scan、aperture_scan、checks、stop_map、move_group、optimize、wave、preview、pupil、pupils。不同操作的额外选项分别为 focus_options、aperture_options、group_move、constraints、optimization 和 wave_options。

```json
{"operation":"wave","project":{},"wave_options":{"method":"sphere_kirchhoff","field_index":0,"wavelength_nm":null,"max_pupil_samples":256}}
```

wavelength_nm=null 表示多色；method=fast 或 sphere_kirchhoff。视场索引和表面索引从零开始；界面显示 S1 从一开始。Rust execute(Request,&AtomicBool) 与 execute_with_progress 共用实现。

## 任务、文件与来源

每个任务带独立 ID 和取消标志；前端用修订号和 AbortController 阻止旧结果覆盖新设计。不同面板可独立运行，切换/编辑导致的失效由各控制器处理。原生项目和 PDF 都通过同目录临时文件原子写入。

源代码采用 Apache-2.0，目录玻璃、字体和依赖分别保留来源及许可证。PDF 包含源项目 SHA-256 和精确附件，显示表格允许舍入，不能替代精确项目。

## 重现与验收

```powershell
cargo test --workspace --release --locked
pnpm test
pnpm build
cargo build -p oos-cli --release --locked
python scripts/validate_coverage.py
python scripts/validate_diffraction.py
```

仓库 docs/ARCHITECTURE.md、COVERAGE.md、DIFFRACTION.md 提供详细实现；docs/TEST_REPORT.md 与 validation-*.json 是实测证据，scripts/ 下为独立验证脚本。独立 RayOptics / NumPy 对照只证明报告列出的模型、采样和算例。干净 Windows 离线安装、代码签名和实际制造验证需要各自证据，不能由编译通过推断。
