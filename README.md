# OpenOpticSim (OOS)

面向摄影与镜头爱好者的本地光学工作室。React + Tauri 2 桌面界面，自研 Rust 双精度顺序追迹核心；界面、命令行使用同一实现。Apache-2.0。

OpenOpticSim is a local-first optical design workbench for photography enthusiasts. It combines a React/Tauri desktop UI with an independent double-precision Rust sequential ray tracer. Windows x64 is the primary platform. The source is Apache-2.0; third-party components retain their own licenses. English issues and contributions are welcome.

![OpenOpticSim 1.0 工作区](docs/screenshots/studio-1.0.png)

**版本：1.5.0。** 源码仓库：[ayr1k/OpenOpticSim](https://github.com/ayr1k/OpenOpticSim)。Windows 安装包通过 [GitHub Releases](https://github.com/ayr1k/OpenOpticSim/releases) 分发，用户无需安装开发工具。

1.5 发布内容已准备，标签与 Release 由维护者发布；下列版本下载链接在 `v1.5.0` 发布并上传附件后生效：

- [Windows x64 安装包](https://github.com/ayr1k/OpenOpticSim/releases/download/v1.5.0/OpenOpticSim_1.5.0_x64-setup.exe)
- [安装包 SHA-256](https://github.com/ayr1k/OpenOpticSim/releases/download/v1.5.0/OpenOpticSim_1.5.0_x64-setup.exe.sha256) · [全部附件校验清单](https://github.com/ayr1k/OpenOpticSim/releases/download/v1.5.0/SHA256SUMS.txt)
- [1.5 发布说明](docs/releases/1.5.0.md) · [版本发布页](https://github.com/ayr1k/OpenOpticSim/releases/tag/v1.5.0)

安装包未签名；干净 Windows 无 WebView2 的断网安装验收尚未完成。具体已验证内容与限制见发布说明。

| 我想做什么 | 入口 |
| --- | --- |
| 第一次使用、搭建并对焦镜头 | [十分钟新手教程](docs/manual/start.md) |
| 查找功能、快捷键与技术解释 | [完整手册](docs/OOS-1.5-使用手册.md) · [离线 HTML](docs/OOS-1.5-使用手册.html) |
| 确认能计算什么、结果如何验证 | [功能与限制](docs/STATUS.md) · [测试报告](docs/TEST_REPORT.md) |
| 使用 400 mm 或其他示例 | [示例与来源](examples/README.md) |
| 报告问题、改进代码或文档 | [贡献指南](CONTRIBUTING.md) · [安全报告](SECURITY.md) |
| 查阅更新、许可和发布流程 | [更新日志](CHANGELOG.md) · [许可审查](docs/THIRD_PARTY_AUDIT.md) · [发布流程](docs/RELEASING.md) |

**当前版本：1.5.0。** 新增约束恢复、结构扩展、镜片级玻璃搜索和可保存搜索断点。保留统一设计任务、覆盖验收、画质报告与计算队列。保留 32 变量优化、镀膜直透、33 种玻璃、教程与 PSF 缓存。经典结构仅在公开测试条件下验证名义几何成像，不等同可制造镜头。详见[设计向导](docs/manual/synthesis.md)与[镀膜说明](docs/manual/coatings.md)。

保留 1.2 功能： 新增变焦补偿轨迹生成、全焦段扫描、多配置联合优化和对焦距离扫描，集中在「调校 → 变焦与对焦工作台」。支持逐视场退化保护、高采样复核、可持久化扫描记录与 CSV 位置表。详见 [工作台教程](docs/manual/study.md) 与 [数值验证](docs/validation-study-1.2.json)。 将既有功能整理为「设计、画质、调校、交付」四组；右侧镜片与摄影设置分别切换，保留独立滚动与布局记忆。增加功能搜索、25 章离线可检索帮助、新手教程和技术文档，以及含矢量剖面、全部处方/材料/非球面参数、分析条件和精确源项目附件的 PDF 技术图纸。

软件内「帮助」、F1 或 Ctrl+K 可查阅与定位功能；也可直接打开 [离线 HTML 完整手册](docs/OOS-1.5-使用手册.html)、[Markdown 手册](docs/OOS-1.5-使用手册.md) 或 [新手教程](docs/manual/start.md)。[功能范围](docs/STATUS.md)和[测试报告](docs/TEST_REPORT.md)说明适用范围与实测证据。既有传感器联动、离轴衍射、扫描、优化、镜组与版本对比全部保留。[0.9 计算范围](docs/COVERAGE.md)继续适用。

## Windows 运行

构建后的安装包位于 `target/release/bundle/nsis/OpenOpticSim_1.5.0_x64-setup.exe`，配置包含 WebView2 离线安装组件、完整手册和许可。也可运行 `target/release/openopticsim-desktop.exe`（需要系统已有 WebView2）。发布版嵌入界面、引擎、PDF 工具与中文字体，不需要 Node、Python、Rust 或本地 HTTP 服务。

下载后可在 PowerShell 中运行 `Get-FileHash .\OpenOpticSim_1.5.0_x64-setup.exe -Algorithm SHA256`，与同版本 `.sha256` 比较。正式附件以对应 Release 的文件为准；源码内的历史本地验收记录不替代下载附件的最新校验清单。

首次使用：

从零搭建可点击“新建项目 → 空白项目 → 添加镜片”。图示折角外形可直接打开“机械环带探索”，调整机械外径和各面有效直径；几何诊断可点击定位。原生项目兼容读取 v1 / v2 / v3；目标像高等新设置使用 v3，另支持扩展 CSV。

1. 打开“单透镜”，观察中心与边缘点列图。
2. 修改右侧“光阑半径”，按 Enter 提交。观察近轴 F 数和光斑；出现通光警告时检查失败明细。
3. 点击“自动对焦”，检查候选指标，再点击“采纳候选”。
4. 切换“专业”编辑圆锥常数、A4–A10 或材料系数。
5. 在“对焦扫描”查看各视场曲线；使用“记录版本 / 版本对比”保存基准，再以统一配置和尺度比较。
6. 「调校 → 约束优化」可填写目标焦距容差、总长、中心厚度、最薄实体与空气间隔约束；候选逐项通过后才可采纳。
7. 保存 `.oos.json` 或导出带材料元数据的 CSV；打开可恢复同一模型。

推荐体验：打开“双胶合探索” → 右侧「摄影与分析」自动对焦 → 采纳候选 →「画质 → PSF / MTF」。从「调校」进入扫描与约束优化，「交付」比较版本和导出技术图纸。「文件与导出」保留另存为及 CSV。具体步骤见内置新手教程。

图纸是名义设计记录，未指定的加工公差、镀膜、表面质量和装配要求不会自动补全，不能直接替代完整加工验收文件。导出固定当前像面，重新计算 33×33 几何；可选逐视场多色衍射。存在失败时明确标为草稿或记录原因，不复用旧结果冒充通过。

算法范围见 [衍射与成像说明](docs/DIFFRACTION.md)。成像仍为局部平面目标，整幅空间变化卷积与畸变映射待实现。

## 开发

需要 Node.js 24、pnpm 11.25、Rust stable。Windows 还需要 Visual Studio C++ Build Tools、Windows SDK、WebView2。

首次搭建环境、验证命令和 PR 要求见 [CONTRIBUTING.md](CONTRIBUTING.md)。本仓库 CI 配置面向 Windows；其他平台尚无安装和运行保证。Node 的 `private: true` 只避免误发布到 npm，不限制源码公开。

```powershell
pnpm install --frozen-lockfile
cargo build -p oos-cli --locked
pnpm dev
```

开发界面为 `http://127.0.0.1:1420/`。Vite 仅在开发时启动 CLI 桥接，计算仍为 Rust；生产 Web 静态预览不具备引擎。

```powershell
pnpm tauri dev       # 原生桌面调试
pnpm release:build   # 测试、路径重映射与 Windows 离线安装包
```

## 命令行与测试

```powershell
cargo run -p oos-cli -- demo
cargo run -p oos-cli -- examples
cargo run -p oos-cli -- analyze examples/singlet.oos.json
cargo run -p oos-cli -- validate examples/singlet.oos.json
cargo test -p openoptics --locked
pnpm test
pnpm build
cargo test --workspace --locked --release
cargo clippy --workspace --all-targets --locked -- -D warnings
```

更完整接口通过 `oos-cli --request` 从 stdin 接收 JSON。参见 [API 与模型约定](docs/ARCHITECTURE.md)。

独立验证仅作为开发依赖，Python 不参与产品运行：

```powershell
python -m venv .tools/validation
.tools/validation/Scripts/python -m pip install -r scripts/requirements-validation.txt
.tools/validation/Scripts/python scripts/validate_rayoptics.py
cargo build -p oos-cli --release
.tools/validation/Scripts/python scripts/benchmark.py
.tools/validation/Scripts/python scripts/validate_diffraction.py
.tools/validation/Scripts/python scripts/validate_mechanics.py
.tools/validation/Scripts/python scripts/validate_workflows.py
```

可直接打开 [双胶合版本对比项目](examples/version-workflow.oos.json)，比较最佳多色对焦与像面后移 0.05 mm。

## 验证证据

- [1.0 发布重建验收](docs/validation-release-1.0.json)：按最新 LICENSE 重建，记录前端 / Rust 回归、400 mm 保存回归、桌面 / 安装向导启动、资源一致性和未完成的安装验收。下载附件的实际校验值见对应 Release。

- [衍射独立验证](docs/validation-diffraction.json)：RayOptics 重追 69,979 条瞳光线、NumPy PSF/复 OTF、直接球面传播与测试图逐像素对照；仅覆盖说明中的标量模型。

- Rust 物理回归包括 Snell 定律、反向折射、全反射、球面解析交点、厚透镜焦距、平板光程、瞳面瞄准、遮光与失败处理、有限物距、非球面导数、多色 RMS、自动对焦、取消和理想圆孔 MTF。
- [RayOptics 交叉验证报告](docs/validation-rayoptics.json)：比较五个教学镜头与有限物距非球面算例的交点、方向和光程。仅验证报告列出的范围。
- [性能报告](docs/benchmark.json)：20 面模型，含进程启动、瞄准、分析和 JSON 输出；结果不代表任意硬件或镜头。
- [衍射性能报告](docs/benchmark-wave.json)：记录逐级采样、三波长 PSF/MTF 与三视场成像的发布版耗时。
- [几何采样收敛](docs/convergence.json)：固定单透镜算例在 9/17/33/65 网格的多色 RMS 变化；不代表衍射收敛。
- 前端测试检查原生文件版本与含引号、换行的 CSV 往返。

## 项目结构

```text
crates/optics/   模型、追迹、分析、优化、标量衍射与局部成像
crates/cli/      JSON 命令行
src/            React 编辑器与图表
src-tauri/      桌面任务调度、取消、安装配置
examples/       可编辑的原创教学处方
scripts/        独立验证、性能测量、图标生成
docs/           约定、限制与真实测量报告
```

基础示例使用合成教学玻璃，不冒充商品材料；400 mm 系列来自用户自行搭建处方及本项目修正，不是佳能官方处方，也不表示达到 400 mm f/2.8。来源与变体见 [examples/README.md](examples/README.md)。材料数据随项目保存并保留来源。

## 许可证与贡献

原创代码、文档和本项目原创资源采用 [Apache-2.0](LICENSE)，保留 [NOTICE](NOTICE)。字体、目录玻璃、依赖和 Microsoft WebView2 分别遵守各自条款，详见 [第三方审查](docs/THIRD_PARTY_AUDIT.md)和 [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt)。不能用本项目许可证替代第三方内容的许可。

贡献前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。计算疑问请提供最小处方、软件版本、波长 / 视场 / 采样和像面条件；不要公开私人或无权再分发的设计文件。
