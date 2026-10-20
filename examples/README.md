# 示例、来源与使用边界

本目录用于学习、回归和复现软件行为。项目原创示例与本项目对用户自建处方的修正按项目 Apache-2.0 分发；材料快照中的第三方数据保留各自来源与许可。没有使用商业镜头拆解图或产品图片作为分发资源，也不声称复刻商品镜头的真实处方。

| 文件 | 来源与目的 |
| --- | --- |
| `singlet.oos.json` | 本项目原创合成单透镜，学习折射、球差与对焦 |
| `doublet.oos.json` | 本项目原创合成双胶合，学习多波长与色差 |
| `triplet.oos.json` | 本项目原创三片式教学结构，不是历史商品处方 |
| `double-gauss.oos.json` | 本项目原创双高斯类型教学结构，不代表某商品设计 |
| `mechanical-annulus.oos.json` | 本项目原创机械环带 / 方边与几何检查示例 |
| `version-workflow.oos.json` | 基于双胶合教学模型的版本与像面对比示例 |
| `400mm-repaired.oos.json` | 用户自行搭建并在开发期间提供的 34 面模型，经本项目重新分配曲率、口径和像距；保留原始模型为命名版本 |
| `400mm-workflow-0.8.oos.json` | 上述修正版的扫描、设计目标、镜组与版本工作流变体 |
| `400mm-coverage-0.9.oos.json` | 上述修正版的传感器覆盖与目标像高变体 |
| `35mm-coverage.oos.json` | 本项目生成的 35 mm 教学模型，用于覆盖缺口与大视场验证，不代表经过校正的商品镜头 |

400 mm 系列不是佳能官方数据，不分发用户参考的商品镜头图片，不代表达到 f/2.8；经验证的修正版约为 400 mm f/16。详见 [修正过程与指标](../docs/REPAIR_400MM.md)。文件内的原始命名版本用于复现故障，不能理解为已通过通光或画质验收。

`docs/fixtures/` 是相应教学 / 用户自建模型的保存、扫描和 UI 回归变体。截图为本项目本地界面与这些模型的输出；保留它们是为了复现实测，不作为商品镜头性能证据。

真实玻璃子集来自 refractiveindex.info 的 CC0 数据快照；项目、材料源 URL 和校验值见 [许可审查](../docs/THIRD_PARTY_AUDIT.md)。新增处方必须说明其原创、授权或可再分发依据，不能把“网上能找到”当作许可。

## v1.1 镜组运动

`24-70mm-zoom.oos.json`：项目自主构建的两正双胶合镜组教学变焦，24/35/50/70 mm 四个配置，4×3 mm 传感器、小光圈；非商业 24–70 mm f/2.8 或全画幅处方。采用已有合成材料快照，按 Apache-2.0 分发。生成器 `scripts/build_zoom_example.py`，独立验证 `scripts/validate_motion.py`，使用方法见 [镜组运动](../docs/manual/motion.md)。

## v1.3 经典拓扑指标设计

`tessar-50mm-1.3.oos.json`、`planar-50mm-1.3.oos.json`、`double_gauss-50mm-1.3.oos.json`、`sonnar-50mm-1.3.oos.json`：由 `synthesis.rs` 原创参数起点和 `scripts/validate_design13.py` 生成，按 Apache-2.0 分发。仅借鉴经典拓扑，未复制厂商数值处方。使用固定 CC0 快照的 N-BK7/N-F2；50 mm、F/8、5° 半视场、三谱线几何复核通过，传感器角落及制造公差未验证。设计族名称用于结构说明，不表示厂商背书。
