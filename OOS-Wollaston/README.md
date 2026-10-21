# OOS-Wollaston v1

OOS-Wollaston 1.0.0 是从 OpenOpticSim 2.0.0 提取的独立 Rust 双精度光学框架。它包含完整的同轴镜头设计计算链，以及带局部坐标系的三维顺序追迹和逐面光线诊断。无需 Tauri、浏览器、Node 或 OpenOpticSim 桌面程序。

## 构建与使用

在此目录运行（需要 Rust stable 工具链；Windows 使用 MSVC 工具链）：

```sh
cargo test --workspace --locked --release
cargo build --workspace --locked --release
cargo doc --workspace --no-deps
```

`target/release/` 生成 `wollaston-cli` 和动态库（Windows 为 `wollaston_ffi.dll`）。CLI 从标准输入读取一个 UTF-8 JSON 请求，标准输出返回结果，错误写标准错误并返回非零状态。

```python
import json, subprocess
p = json.load(open('examples/singlet.oos.json', encoding='utf-8'))
request = dict(operation='analyze', project=p, sampling=33)
result = subprocess.run(['target/release/wollaston-cli.exe'],
                        input=json.dumps(request), text=True, capture_output=True, check=True)
print(json.loads(result.stdout)['paraxial'])
```

Python 嵌入：将 `python/wollaston.py` 放入 Python 搜索路径，使用发行 DLL 的绝对路径。

```python
from wollaston import Wollaston
engine = Wollaston('target/release/wollaston_ffi.dll')
print(engine.execute(request)['paraxial'])
```

Rust：依赖 `wollaston = { package = "oos-wollaston", path = ".../crates/core" }`，调用 `wollaston::execute` 或具体模块的类型化函数。C/C++：包含 `include/wollaston.h`，链接 `wollaston_ffi.dll.lib`，将 DLL 放在应用可加载的位置。示例见 `examples/embed.c`。

## 提供的能力

- 材料色散、处方验证、几何检查、近轴性质、真实光阑瞄准、顺序折射、通光失败分类。
- 同轴多视场点列图、像差、PSF/MTF、有限平面目标预览、对焦/光圈/变焦扫描、联合优化、设计搜索和断点。
- 三维显式光线、表面和镜组刚体位姿、可倾斜传感器、附加圆/椭圆/矩形/环形口径、逐面光程/法线/入射与折射方向。
- Rust、JSON CLI、C ABI 1、Python ctypes 接口。三维批处理可限制线程并保持输出顺序，计算支持取消。

请先阅读 [API 与坐标约定](docs/API.md)、[数值范围和验证](docs/VALIDATION.md) 和 [迁移说明](docs/MIGRATION.md)。独立版本号从 1.0.0 开始，和桌面 2.0.0 分开。

## 重要边界

三维位姿只用于 `trace_spatial`。其他操作仍是原有旋转对称模型；API 会拒绝将三维配置与其他操作混用。此版本没有三维瞳瞄准、偏心系统波前/MTF、非顺序追迹、GPU 核心或完整场景渲染。几何通光比例不等于精确相对照度。示例是教学设计，400 mm 不是商业镜头官方处方。

此文件夹可整体复制后独立建仓。它自身没有指向父仓库的构建依赖。OpenOpticSim 当前通过本地路径依赖这一份源码，未来单独发布后应改为锁定明确版本或提交，不维护两份算法分支。

原创代码 Apache-2.0。玻璃目录保留 CC0 和原始来源；第三方依赖见 `THIRD_PARTY_NOTICES.txt`、`docs/dependencies.json`。具体发布附件以 SHA256SUMS 和构建记录为准。
