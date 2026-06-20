# Aerospace & Defense

Part of [sfi-platform](https://github.com/StructForIndustry/sfi-platform) · Domain: **Aerospace & Defense**  
Split repo (future): `StructForIndustry/sfi-domain-aerospace`

## Overview

Flight software, avionics integration, mission simulation, and guidance-related compute — with reliability and certification-oriented boundaries.

## Focus

| Area | Stack | Notes |
|------|-------|-------|
| Avionics / sensors | Zig `hal-ext` | buses, sensors, actuator interfaces |
| Mission software | Rust `core-bus` | state machines, redundancy, secure comms |
| Simulation | Julia plugins | physics, orbital mechanics, Monte Carlo |
| Accelerated sim | Mojo plugins | GPU-heavy simulation kernels (optional) |

## Reliability over raw speed

Schedulers emphasize determinism, failover, and audit — not maximum throughput.

## Directory layout

```
aerospace-defense/
├── manifest.yaml
├── hal-ext/
├── plugins/
├── profiles/
├── compliance/         # DO-178C / export-control placeholders
└── examples/
```

## Profiles

| Profile | Scheduler | Notes |
|---------|-----------|-------|
| `flight-control` | cyclic RT, watchdog | critical control loop |
| `mission-simulation` | batch + reproducible seeds | offline sim |

## Dependencies

- `core/contracts`, `core/hal-rs`, `core/core-bus`, `core/plugin-host` (required)
- `core/math-kernel` (recommended)

**Must not** depend on other `domains/*`.

## Status

| Component | Status |
|-----------|--------|
| manifest + docs | ✅ scaffold |
| plugins | 🔲 planned |

## Switch domain

[`sfi.code-workspace`](../../sfi.code-workspace) · [Domains table](../../README.md#domains)
