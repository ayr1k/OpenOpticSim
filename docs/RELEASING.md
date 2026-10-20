# OOS 本地检查与发布流程

源码仓库为 https://github.com/ayr1k/OpenOpticSim 。以下流程只构建、验证和整理本地文件，Git 提交、标签、推送和 Release 均由维护者操作。

## 1. 检查公开内容

```powershell
python scripts/audit_public.py --self-test
python scripts/audit_public.py
```

扫描 `git ls-files --cached --others --exclude-standard`：包含未跟踪文件，也包含已经跟踪但后来被忽略的文件。报告在被忽略的 `artifacts/public-audit.json`，命中只写文件、行号、规则与指纹，不回显密钥。规则覆盖常见令牌、私钥、带密码 URL、凭据文件名、个人主目录与 Git 大文件；不替代人工审阅或专业密钥扫描，忽略文件和二进制像素不作文本扫描。脚本扫描当前工作文件，不声称检查了全部提交历史。

`target`、`node_modules`、本地工具、`tmp`、`output`、`artifacts` 和签名材料均不提交。截图和字体是必要源码资源；新增截图应检查可见私人信息及 EXIF / PNG 元数据。不要删除上游的公开版权署名或邮箱来“消除命中”。

复核 [第三方许可](THIRD_PARTY_AUDIT.md)、[示例来源](../examples/README.md)。重新安装依赖后运行 `python scripts/notices.py`；检查补充许可例外与锁文件，不只看脚本退出码。

## 2. 构建与回归

```powershell
pnpm install --frozen-lockfile
pnpm test
pnpm build
cargo test --workspace --locked --release
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build -p oos-cli --locked --release
```

独立参考环境与命令见 [贡献指南](../CONTRIBUTING.md)。首次发布或改计算链时运行 `validate_rayoptics.py`、`validate_diffraction.py`、`validate_mechanics.py`、`validate_workflows.py`、`validate_optimization.py`、`validate_aiming.py`、`validate_design.py`、`validate_saved_design.py`、`validate_coverage.py`、`validate_motion.py`、`validate_study.py`；脚本生成的报告应审阅差异，不能只覆盖旧结果。

`validate_coverage.py` 不依赖外部个人文件。如另需验证自己的原始处方未改变，可设置 `OOS_ORIGINAL_PROJECT` 指向本机文件；报告只记录哈希，不写入路径。不要提交该环境配置。

PDF 有关改动使用 `node scripts/render_drawings.mjs` 和 `python scripts/validate_pdf.py`，需另装 PDF 验证依赖与 Poppler。软件文档见 [完整测试记录](TEST_REPORT.md)。

## 3. 构建安装包并整理资产

```powershell
pnpm release:build
python scripts/stage_release.py
```

整理到 `artifacts/release-1.3.0/`：安装包和单独校验文件、SHA256SUMS、JSON manifest / build-report、独立 HTML 手册、文档 / 示例 ZIP、独立发布说明及许可文件，共 11 个附件。脚本验证版本、许可资源、字体和示例资源一致性；生成报告仅写入被忽略的附件目录，不再改写源码内的历史报告。普通程序 exe 依赖 WebView2 和相邻资源，不将它单独作为“便携版”发布。

`release:build` 使用 Node 辅助脚本：从当前环境动态映射用户目录、Cargo / Rustup 缓存和工作区为通用路径，并用 MSVC `/PDBALTPATH` 只保留 PDB 文件名；不把任何人的绝对目录写进源码配置。它在相同标志下重新运行 Rust 测试、构建 CLI 与 NSIS，然后检查两个 exe 的 ASCII / UTF-16 个人主目录路径。`stage_release.py` 再次检查，发现残留会拒绝整理。普通 `pnpm tauri build` 可用于本地调试构建，不作为通过该隐私检查的发布方法。

已有 `CARGO_ENCODED_RUSTFLAGS` 会保留并追加映射。若只设置了自定义 `RUSTFLAGS`，脚本要求维护者先转换为编码参数，避免默默改写有空格的编译选项。PDB 文件留在本地构建目录，不进入发布资产；映射不能替代其他凭据与资源审查。

技术依据：[Rust 路径重映射](https://doc.rust-lang.org/rustc/remap-source-paths.html)、[MSVC PDB 替代路径](https://learn.microsoft.com/en-us/cpp/build/reference/pdbaltpath-use-alternate-pdb-path?view=msvc-170)。

若目录中已有文件，脚本仅在它生成的清单范围内更新同名资产，不递归删除目录或用户文件。发布资产以最终安装包真实 SHA-256 为准；改动资源后必须重建安装包，不能只改 manifest。`docs/release-1.0.json` 保留早期候选包的历史验收记录；本次实际下载包以附件 `manifest.json` / `build-report.json` 为准。[releases/1.2.0.md](releases/1.2.0.md) 会复制为可粘贴到 GitHub 的独立发布说明。

本机启动验证输出可存为 `artifacts/startup-release-1.2.json`。整理脚本只接受 exe 哈希相同的启动记录，不能把旧二进制的测试套用到新文件。

在没有开发工具且没有 WebView2 的干净 Windows x64 环境断网安装，检查新建、示例、保存重开、取消、帮助与 PDF 原生对话框，并记录硬件 / 系统 / 实测结果。当前这项未完成；已完成的本机测试不能代替它。代码签名应使用独立保管的密钥，目前安装包未签名。

## 4. GitHub Desktop 与 Release 的人工步骤

1. 在 GitHub Desktop 检查并提交这批发布文档 / 工具改动，Push origin，并检查对应提交的 Actions 结果。
2. 在干净工作区运行构建和整理。若是在提交前预先准备了安装包，提交后再运行一次 `python scripts/stage_release.py`，刷新文档包、提交号和校验文件；如果编译源码、依赖、配置或打包资源也发生变化，先重新构建。
3. 仅当 `manifest.json` 的 `source_worktree_dirty` 为 false、`source_snapshot_finalized` 为 true 时，将 `source_commit` 对应提交标为 `v1.2.0`。本地有未提交改动时该字段只是基准提交，不能把它误当完整待发布源码。整理过程不会再改变被跟踪文件，因此刷新后无需为生成元数据再创建一次提交。
4. 在网页创建 Release，选择该标签，粘贴独立发布说明，上传 11 个附件并检查附件校验。实际发布日以 GitHub 页面为准；文档中的构建日期不冒充发布日期。
5. GitHub 自动提供标签源码归档；不要向 Git 提交安装包、WebView2 运行时或 `node_modules`。
6. 启用 Issues、私密漏洞报告与适合个人维护的分支保护；本地脚本不核实线上 CI，不能把未知状态写成通过。

官方参考：[导入本地仓库](https://docs.github.com/en/migrations/importing-source-code/using-the-command-line-to-import-source-code/adding-locally-hosted-code-to-github)、[Releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases)。
