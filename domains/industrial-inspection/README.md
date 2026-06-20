# Industrial Inspection

Part of [sfi-platform](https://github.com/StructForIndustry/sfi-platform) · Domain: **Industrial Inspection**  
Split repo (future): `StructForIndustry/sfi-domain-industrial`

## Overview

Production-line vision, defect detection, dimensional measurement, SPC, and MES integration. Primary active domain for StructForIndustry.

## Focus

| Area | Stack | Notes |
|------|-------|-------|
| Acquisition | Zig `hal-ext` | GigE/USB cameras, PLC, motion triggers |
| Orchestration | Rust `core-bus` | line recipes, beat time, drop-frame policy |
| Algorithms | Julia plugins | traditional CV, lightweight ML, SPC stats |
| Inference | Mojo `plugins/ai-infer` | GPU/NPU defect models (optional phase) |

## Latency target

| Mode | Target |
|------|--------|
| CPU pipeline | end-to-end &lt; 50 ms @ 1080p |
| GPU inference | end-to-end &lt; 20 ms @ 1080p |

## Directory layout

```
industrial-inspection/
├── manifest.yaml       # domain pack metadata
├── hal-ext/            # industrial HAL extensions
├── plugins/            # domain business plugins
├── profiles/           # default recipes & deployment profiles
├── compliance/         # audit templates, traceability hooks
└── examples/           # end-to-end demos
```

## Profiles

| Profile | Scheduler | HAL | Notes |
|---------|-----------|-----|-------|
| `line-realtime` | drop stale frames, keep latest | triggered capture | default for AOI |
| `lab-batch` | queue all frames | free-running camera | R&D and golden-set eval |

See [`profiles/`](profiles/) for field-level configuration.

## Dependencies

- `core/contracts` (required)
- `core/hal-rs`, `core/core-bus`, `core/plugin-host` (required)
- `core/math-kernel` (recommended)
- `plugins/vision-2d`, `plugins/ai-infer` (optional)

**Must not** depend on other `domains/*` packages.

## Status

| Component | Status |
|-----------|--------|
| manifest + docs | ✅ |
| profiles + hot reload API | ✅ |
| plugins/defect-detect | ✅ sidecar + mock + profile default |
| plugins/spc-metrics | ✅ Julia lib + bus `spc.metrics` |
| plugins/mes-reporter | ✅ REST + bus integration |
| hal-ext/line-publisher | ✅ triggered shm frames |
| examples/aoi-line-demo | ✅ full stack script |
| Web preview (`/` on sfi-bus) | ✅ |
| hal-ext PLC/GigE | 🔲 planned |

## Related domains

Switch workspace folder to another domain in [`sfi.code-workspace`](../../sfi.code-workspace).
