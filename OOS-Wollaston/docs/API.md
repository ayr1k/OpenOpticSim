# API v1 与数值约定

内部长度 mm，角度 rad，折射率无量纲；波长接口 nm，Sellmeier 求值使用 μm。全局 Z 是名义传播方向，X/Y 为横向。曲率为 1/mm，正曲率中心朝 +Z。原有处方 JSON schema 1/2/3 继续读取，材料系数随处方携带。

## 执行入口

`execute(Request, &AtomicBool)` 返回 `Result<Value, String>`；`execute_with_progress` 接收优化进度回调，在调用线程执行。取消令牌为 true 后不可用结果不会当作有效候选返回。具体操作可通过 Rust 模块的类型化 API 调用；用 `cargo doc --workspace --no-deps` 查阅类型与字段。

JSON 请求包含 `operation`、`project` 和该操作的选项。主要操作：`validate`、`geometry`、`analyze`/`trace`、`field_plan`、`wave`、`preview`、`pupil`/`pupils`/`stop_map`、`autofocus`、`focus_scan`、`aperture_scan`、`move_group`、`motion`、`motion_study`、`checks`、`optimize`、`synthesize`/`synthesize_step`、`coatings`、`trace_spatial`。选项定义见 `Request` 和各模块；既有操作没有删除。CLI/C 请求最大 4 MB。

`PreparedTrace::new(&Project, wavelength_nm)` 借用不可变处方并预计算顶点、介质折射率和发射平面；同一波长的光束复用它。Rust 借用期间不能修改处方。它没有跨请求全局缓存；修改任意参数或波长后重新创建。不要缓存一个旧项目的结果来代表新项目。

## 三维请求

```json
{
  "operation": "trace_spatial",
  "project": "此处填完整处方对象，不是字符串",
  "spatial_options": {
    "poses": [{"offset_mm":[0.1,0,0],"rotation_rad":[0,0.01,0]}],
    "groups": [],
    "apertures": [{"kind":"ellipse","x":5,"y":4}],
    "detector": {"offset_mm":[0,0,0],"rotation_rad":[0,0,0]},
    "rays": [{"origin_mm":[0,1,-100],"direction":[0.001,0,1],"wavelength_nm":587.5618}],
    "threads": 4
  }
}
```

`poses[i]` 对应零基表面 i，缺省单位位姿。平移是相对名义顶点的全局位移；旋转矩阵为 Rz*Ry*Rx，绕本面顶点。局部点 q 映射到 global_vertex + R*q。单位方向在输入后归一化。

镜组项 `{start,end,pose}` 的闭区间都是零基表面编号。先应用表面位姿，再绕组首名义顶点整体旋转并平移；重叠组会报错。传感器位姿相对原始像面中心，与镜组变换独立。

附加口径与原始有效半径取交集，不扩大光学表面。ellipse/rectangle 的 x/y 是半宽；annulus 使用 inner/outer 半径。机械环带不参与折射。任何反射路径仍不属于顺序折射模型；遇全反射终止。

结果按输入顺序返回，含 `ray_index`、波长、全局 `points`、逐面 `hits`、状态、失败面编号、总光程及传感器局部交点。Hit 包括局部/全局位置、法线、入射/出射向量、n/n′、入射/折射角和累计光程。失败不伪造后续交点。状态区分 missed、blocked、total_internal_reflection、non_convergent；材料和无效输入在追迹前报错。

调用方显式选择 rays；本入口没有自动光阑瞄准。最多 100000 条光线，线程请求限制 1..64 且不超过可用逻辑处理器，结果保持原序。取消检查在表面间执行。原有几何/衍射操作保留各自的资源策略，threads 仅控制本入口。

## C ABI / Python 内存和并发

C 使用 cdecl 和 ABI 整数 1。`wollaston_execute` 同步执行，可并行调用独立请求。输入为 NUL 结尾 UTF-8，调用期间保持可读。返回 JSON 信封 `{"ok":true,"result":...}` 或 `{"ok":false,"error":"..."}`。无异常跨 ABI 传播；Rust panic 被捕获，非法外部指针不在可恢复保证内。

每个返回指针必须用 `wollaston_free` 释放一次，不能用调用方的 free。`wollaston_cancel_new` 创建一次性取消令牌，其他线程调用 cancel；所有使用该令牌的 execute 返回后才可 cancel_free。空令牌表示不请求取消。Python 包装器自动释放结果，`with engine.cancellation() as token:` 可配合工作线程；退出 with 之前必须 join 工作线程。对象关闭后不可使用令牌。

版本边界：本版稳定的是 ABI 1 函数和已记录 JSON 字段的含义；未来加可选字段保持兼容，破坏性变化更换主版本。不要依赖内部结构布局或未公开 Rust 实现细节。所有输入来源仍应由嵌入应用限制大小和运行预算。
