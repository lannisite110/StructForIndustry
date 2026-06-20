# StructForIndustry 分阶段开发路线图

> Repo: **StructForIndustry** (monorepo)  
> **唯一领域**: `domains/industrial-inspection`（工业 AOI 边缘检测平台）

> **收敛声明 (2026-06)**: 平台已从「六领域通用框架」**钉死**为单一工业 AOI 平台。
> 其余五领域 scaffold（robotics / quant / aerospace / biomed / cloud-edge）**已删除**，
> 不再保留 README / manifest。core/* 仍是可复用地基，但短期内不再开第二领域。
> 主线只做一件事：**让平台产出三张表** ——
> 1. 换型曲线（5/10/20 OK 样本）
> 2. 推理延迟表（CPU vs GPU，目标 <20ms）
> 3. 光照 ablation（实验台数据）

## 总览

| 阶段 | 名称 | 周期（估） | 核心交付 | 验收标准（摘要） |
|------|------|------------|----------|------------------|
| 0 | 契约与仓库基座 | 2–4 周 | schema、ABI、CI、文档 | 假插件通过契约集成测试 |
| 1 | 核心主库 MVP | 6–10 周 | hal-rs、core-bus、plugin-host | 单相机帧进入总线 |
| 2 | 数学核 + 技术插件 | 4–6 周 | math-kernel、vision-2d | Julia 进程处理共享内存帧 |
| 3 | 工业标杆领域包 | 6–10 周 | industrial-inspection 端到端 | AOI demo：检测 + API + 审计 |
| 4 | AI 推理插件 | 4–6 周 | ai-infer (ONNX + `ort`) | 同 Task 类型替换 CPU 推理；OK-only 模型落地 |
| 5 | 边缘部署收尾 | 4–8 周 | Docker/边缘 runtime、三张表 | 单机边缘可部署 + 换型/延迟/光照报告 |

**合计（到 Phase 3 可演示产品）**: 约 **4–6 个月**

> Phase 5 不再做「第二领域试点」与「生态拆库」；冻结多领域扩张，集中灌内容。

---

## Phase 0 — 契约与仓库基座

**目标**: 定「宪法」，所有后续语言栈在同一契约上对齐。

| 模块 | 语言 | 交付物 | 内容要点 |
|------|------|--------|----------|
| `core/contracts` | Cap'n Proto + C | `schema/*.capnp` | `Frame`, `Task`, `Result`, `PluginManifest`, `TimingMetrics` |
| `core/contracts/abi` | C | `sfi.h` | in-process 插件：`sfi_init` / `sfi_process_task` / `sfi_shutdown` |
| 版本策略 | — | `apiVersion` 文档 | v0 允许修订；v1 起 semver 与兼容矩阵 |
| CI | GitHub Actions | `.github/workflows/` | schema 校验、Rust fmt/clippy、Zig build、Julia 语法检查 |
| 文档 | Markdown | `docs/`, README 导航 | 架构图、领域切换说明、CONTRIBUTING |
| 假插件 | Rust | `tools/fake-plugin` | 消费 mock `Task`，验证加载与 IPC |

**里程碑**: `cargo test` + `zig test` + 契约兼容性测试全绿。

**人力（估）**: 1 架构 + 1 Rust，兼职 Zig。

---

## Phase 1 — 核心主库 MVP

**目标**: 硬件采集 → 共享内存 → 消息总线，无业务算法。

| 模块 | 语言 | 交付物 | 内容要点 |
|------|------|--------|----------|
| `core/hal-rs` | Zig | 库 + 采集进程 | `DeviceRegistry`, `FramePool`, `CaptureStream`；先支持 **V4L2/USB** 或单一 GigE SDK |
| `core/core-bus` | Rust | 库 + 守护进程 | 消息主题：`frame.new`, `task.done`, `plugin.health`；基础 `TaskScheduler` |
| `core/plugin-host` | Rust | 运行时 | manifest 加载、`apiVersion` 协商、子进程 supervisor、崩溃重启 |
| IPC | — | 共享内存 + Cap'n Proto | 像素走 handle；控制面走 Unix socket |
| `core/core-bus` | Rust | REST/gRPC 薄层 | 健康检查、帧统计、配置只读 API |
| 配置 | YAML | `profiles` 机制 | 调度策略：队列深度、丢帧策略（为工业 profile 铺路） |

**里程碑**: 单路 1080p 相机稳定 30fps，`frame.new` 可在日志/ API 中观测，插件崩溃 30s 内恢复。

**人力（估）**: 1 Zig + 2 Rust，约 6–10 周。

**风险**: 相机 SDK 选型 — 先开源/通用协议，厂商 SDK 放 `hal-ext`。

---

## Phase 2 — 数学核 + 技术插件

**目标**: 打通 **Rust 总线 ↔ Julia 算法进程** 的标准路径。

**状态 (2026-06)**: ✅ 完成 — plugin wire、TaskScheduler、**task.done 广播**、**plugin supervisor**、Julia sidecar、mock E2E、`sfi-cli` v0.1。

| 模块 | 语言 | 交付物 | 内容要点 |
|------|------|--------|----------|
| `core/math-kernel` | Julia | 包 +  sidecar 服务 | 图像几何、滤波、形态学、基础统计；gRPC 或 Cap'n Proto |
| `plugins/vision-2d` | Julia/Rust | 技术插件 | 灰度阈值、边缘、连通域、简单缺陷逻辑 |
| Arrow 可选 | — | 中间格式 | 大批量离线分析路径（非热路径必选） |
| `tools/sfi-cli` | Rust/Shell | v0.1 | `domain list`, `domain use`, `plugin new` 脚手架 |
| 集成测试 | — | `tests/e2e/` | 合成帧 → Julia 处理 → `Result` 回总线 |

**里程碑**: 端到端延迟可测；Julia 插件与 Rust 插件可互换注册到 `plugin-host`。

**人力（估）**: 1 Julia + 1 Rust，约 4–6 周。

---

## Phase 3 — 工业标杆领域包（杀手级 Demo）

**目标**: **industrial-inspection** 成为可对外演示的 AOI 最小产品。

**状态 (2026-06)**: 🚧 收尾中 — 配方热更新、MES、line-publisher、AOI Web + SPC trend、audit JSONL、lab-batch profile、1080p bench、plc-trigger 模拟；真机 GigE/PLC 待做。

| 模块 | 路径 | 交付物 | 内容要点 |
|------|------|--------|----------|
| HAL 扩展 | `domains/industrial-inspection/hal-ext` | Zig | 产线相机触发、PLC 握手（或 GPIO 模拟触发） |
| 业务插件 | `domains/.../plugins/defect-detect` | Julia | 表面缺陷传统 CV 流水线 |
| 业务插件 | `domains/.../plugins/spc-metrics` | Julia | 灰度/SPC 指标、趋势窗口 |
| 配方 | `profiles/line-realtime.yaml` | 配置 | ROI、阈值、节拍、丢帧策略 |
| 合规钩子 | `compliance/` | 模板 | 配方变更审计、结果保留策略 |
| MES 对接 | `plugins/` 或 domain | Rust | REST/MQTT 上报：`OK/NG`、缺陷图、批次号 |
| 示例 | `examples/aoi-line-demo` | 文档+脚本 | 一键启动：采集→检测→API→简单 Web 预览 |
| UI（可选） | — | Web | 实时画面 + NG 列表（可用现有轻量框架） |

**里程碑**:

- 1080p 单路：CPU 流水线端到端 **< 50ms**（P95）
- 换阈值热更新，无需重启采集
- NG 结果可追溯（时间戳、配方版本、源图 handle）

**人力（估）**: 2 全栈（Rust+Julia）+ 1 视觉算法，约 6–10 周。

**对外叙事**: 「StructForIndustry 工业检测 MVP」— GitHub README 视频/GIF。

---

## Phase 4 — AI 推理插件

**目标**: 在 **不改变 Task/Result 契约** 的前提下，用 GPU 替换或增强检测后端。

| 模块 | 语言 | 交付物 | 内容要点 |
|------|------|--------|----------|
| `plugins/ai-infer` | ONNX RT (`ort`) | 推理插件 | 加载 ONNX；`Task.type = infer.onnx`（默认 reference stub，`--features onnx` 走真 ORT） |
| OK-only 模型 ✅ | Rust | 模型 + 工具 | PatchCore/EfficientAD-lite（`anomaly.rs`）：仅用 OK 样本标定，per-cell 最近邻距离打分，仿射光照不变；`sfi-anomaly calibrate/score/dump/report`，`SFI_ANOMALY_MODEL` 接入；E2E：缺陷→NG、OK→OK |
| 模型管线 | — | 文档 | 训练/导出 → ONNX → ai-infer |
| 资源管理 | Rust | plugin-host 扩展 | GPU 显存配额、批推理、队列合并 |
| A/B 路径 | industrial | 配置开关 | 同一工位：传统 CV vs 深度学习对比 |
| 基准 | `tests/bench/` | 报告 | CPU vs GPU 延迟、吞吐 |

**里程碑**: 1080p 推理端到端 **< 20ms**（目标硬件上）；Julia 检测与 ONNX 可配置切换。

**人力（估）**: 1 ML + 1 Rust，约 4–6 周。

**主路径**: ONNX + Rust `ort`。Mojo 不在主线（hobby 可选）。

---

## Phase 5 — 边缘部署收尾 + 三张表

**目标**: 从「单机 AOI」升级为「可部署的边缘产品」，并输出对外可信的量化结果。

| 模块 | 路径 | 交付物 | 内容要点 |
|------|------|--------|----------|
| 边缘 runtime | `domains/industrial-inspection` | Rust + Zig | 轻量部署、远程配置、健康上报 |
| 部署 | `examples/deploy` | Docker | 单机 compose（已有 docker-demo-smoke） |
| 可观测 | — | metrics/logs | Prometheus：fps、延迟、队列深度、插件重启次数（已有 metrics 雏形） |
| **三张表** ✅ | `docs/reports/` | 报告 | 换型曲线 / 推理延迟表 / 光照 ablation（`tools/scripts/anomaly-reports.sh` 生成，合成帧；待替换为实验台数据） |

**里程碑**: 工业边缘节点可独立部署；产出换型、延迟、光照三张表（首版已落地于 `docs/reports/`，数据源为合成帧，接实验台后即为真实结果）。

**人力（估）**: 1–2 平台，约 4–8 周。

> **冻结**: 不做第二领域试点、不做生态拆库 / 插件模板市场。多领域扩张是负债，
> 集中资源把工业 AOI 一条线灌满内容（OK-only 模型、误报表、延迟表）。

---

## 技术栈分工（各阶段参与度）

| 阶段 | Zig | Rust | Julia | ONNX/`ort` |
|------|-----|------|-------|------------|
| 0 | 低 | 高 | 低 | — |
| 1 | 高 | 高 | — | — |
| 2 | 低 | 高 | 高 | — |
| 3 | 中 | 高 | 高 | — |
| 4 | — | 高 | 中 | 高 |
| 5 | 中 | 高 | 中 | 中 |

> Zig/HAL「够用就好」（能采到帧即可），不为采集层再投半年。Mojo 不在主线。

---

## 依赖关系简图

```mermaid
flowchart LR
  P0[Phase 0 契约] --> P1[Phase 1 核心 MVP]
  P1 --> P2[Phase 2 数学核]
  P2 --> P3[Phase 3 工业 Demo]
  P3 --> P4[Phase 4 AI 推理]
  P4 --> P5[Phase 5 边缘部署 + 三张表]
```

---

## 资源与优先级（建议）

| 优先级 | 投入 | 说明 |
|--------|------|------|
| P0 | 必须 | Phase 1 采集 + 总线 — 一切的地基 |
| P1 | 强烈建议 | Phase 3 工业 Demo — 验证商业叙事 |
| P1 | 强烈建议 | Phase 4 OK-only 模型 + 延迟表 — 真正的差异化与硬证据 |
| P2 | 建议 | Phase 5 边缘部署 + 三张表 |

---

## 相关文档

- [根 README](../README.md)
- [CONTRIBUTING](../CONTRIBUTING.md)
- [工业检测领域](../domains/industrial-inspection/README.md)
