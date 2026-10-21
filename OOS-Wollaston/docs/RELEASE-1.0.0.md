# OOS-Wollaston 1.0.0

独立 Rust 双精度顺序光学框架，首次与 OpenOpticSim 2.0.0 一起交付。原创代码 Apache-2.0；日期 2026-10-21，实际发布日由维护者确定。

## 可以完成什么

在自己的应用中计算镜头的近轴性质、实际顺序光路、瞳瞄准、像差、标量衍射、成像预览、设计优化与变焦研究。三维显式光线入口支持局部位姿、镜组变换、附加口径、倾斜像面和逐面诊断。Rust、JSON CLI、C ABI 1 与 Python ctypes 使用同一算法。

## 开始使用

源码用户解压 source.zip 后，在其中执行 `cargo test --workspace --locked --release` 和 `cargo build --workspace --locked --release`。

Windows 嵌入用户解压 SDK，按 README、docs/API.md 配置 `wollaston_ffi.dll`、导入库和 C 头文件。Python 不要求额外 pip 包，用 python/wollaston.py 加载 DLL。C 字符串必须用库提供的 free 释放，取消令牌在调用结束后释放。

## 附件

| 文件 | 用途 |
| --- | --- |
| `OOS-Wollaston-1.0.0-source.zip` | 可独立建仓的完整源目录、锁文件、测试、文档和许可 |
| `OOS-Wollaston-1.0.0-windows-x64-sdk.zip` | DLL、导入库、CLI、C 头文件、Python 包装器、示例和文档 |
| `OOS-Wollaston-1.0.0-release-notes.md` | 本说明 |
| `manifest.json`、`SHA256SUMS.txt` | 附件 SHA-256、验证和来源记录 |

## 范围与验证

保留原同轴设计/衍射能力。三维入口只做显式光线的顺序几何追迹，不将同轴 PSF/MTF 应用于偏心系统；不含非顺序、GPU、完整场景渲染。几何通光不等于精确相对照度。

本地验证涵盖原有数值回归、三维解析/坐标协变/并行一致性、独立 RayOptics 参考和 CLI/C/Python 一致性。构建记录区分实际通过与尚未执行的测试；远程 CI 由维护者发布后检查。Windows DLL 未签名，其他平台本版提供源码及 CI 配置，不附未经验证的预编译二进制。

独立建仓时上传解压后的源目录内容即可，不需要父桌面仓库、node_modules 或 target。后续桌面项目应固定框架版本或提交。两者分别维护版本号与发布说明。
