# 从 OpenOpticSim 1.5 到框架 v1

原 crates/optics 中的算法已迁入 crates/core，包名 oos-wollaston，Rust 库名 wollaston。桌面仓库保留 openoptics 兼容 facade，因此已有分析/优化/扫描调用保持不变。

独立框架不依赖 UI 状态、React、Tauri 或父目录；玻璃快照在 data/，验证用处方在 examples/。Project 不解释桌面 workspace，桌面保存的三维实验配置需要显式转为 spatial_options 后执行 trace_spatial。

独立复制本目录即可构建；不要复制 target/、父 .git 或本机虚拟环境。新建仓库后，桌面仓库应选择版本发布依赖或固定提交依赖，在其 CI 运行兼容回归。当前交付仍使用本地路径依赖，便于首次独立建仓前验证。

同轴设计搜索断点保留原 1.5.0-search1 标识，因为这次没有改变计划/评分算法；重新载入候选仍执行原有验证。三维实验配置另存 workspace.spatial，不写进同轴历史版本的光学处方。禁止用同轴版本比较声称比较了偏心系统。
