# Autonomous Driving & Robotics

Part of [sfi-platform](https://github.com/StructForIndustry/sfi-platform) · Domain: **Autonomous Driving & Robotics**  
Split repo (future): `StructForIndustry/sfi-domain-robotics`

## Overview

Sensor fusion, localization, planning, and low-latency control for autonomous vehicles and robotic systems.

## Focus

| Area | Stack | Notes |
|------|-------|-------|
| Sensors | Zig `hal-ext` | LiDAR, cameras, IMU, CAN/EtherCAT |
| Safety core | Rust `core-bus` | deterministic scheduling, redundancy hooks |
| Algorithms | Julia plugins | fusion, planning prototypes |
| Acceleration | Mojo `plugins/ai-infer` | perception model serving |

## Latency target

| Path | Target |
|------|--------|
| Perception → planner | &lt; 100 ms (platform-dependent) |
| Control loop | sub-ms to 10 ms (hard real-time profile) |

## Directory layout

```
autonomous-robotics/
├── manifest.yaml
├── hal-ext/
├── plugins/
├── profiles/
├── compliance/         # functional-safety hooks (ISO 26262 placeholders)
└── examples/
```

## Profiles

| Profile | Scheduler | Notes |
|---------|-----------|-------|
| `vehicle-perception` | strict priority, no drop on safety topics | ADAS / perception stack |
| `robot-arm` | cyclic RT | manipulator control |

See [`profiles/`](profiles/).

## Dependencies

- `core/contracts`, `core/hal-rs`, `core/core-bus`, `core/plugin-host` (required)
- `core/math-kernel`, `plugins/ai-infer` (recommended)

**Must not** depend on other `domains/*`.

## Status

| Component | Status |
|-----------|--------|
| manifest + docs | ✅ scaffold |
| hal-ext | 🔲 planned |
| plugins | 🔲 planned |

## Switch domain

Open another folder in [`sfi.code-workspace`](../../sfi.code-workspace) or see [root README](../../README.md#domains).
