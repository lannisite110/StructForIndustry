# Cloud & Edge Computing

Part of [sfi-platform](https://github.com/StructForIndustry/sfi-platform) · Domain: **Cloud & Edge Computing**  
Split repo (future): `StructForIndustry/sfi-domain-cloud-edge`

## Overview

Multi-tenant edge runtime, cloud orchestration adapters, and intelligent analytics — the platform carrier for deploying other domain packs at scale.

## Focus

| Area | Stack | Notes |
|------|-------|-------|
| Edge runtime | Zig + Rust | minimal agent, secure sandbox |
| Control plane | Rust `core-bus` | tenancy, RBAC, fleet config |
| Analytics | Julia plugins | batch intelligence, digital twin |
| Inference fleet | Mojo `plugins/ai-infer` | GPU pool serving |

## Profiles emphasize elasticity

Backpressure, queue depth, and tenant quotas — not single-machine hard RT.

## Directory layout

```
cloud-edge/
├── manifest.yaml
├── hal-ext/            # virtual devices, K8s/device plugins
├── plugins/
├── profiles/
├── compliance/         # multi-tenant audit, data residency
└── examples/
```

## Profiles

| Profile | Scheduler | Notes |
|---------|-----------|-------|
| `edge-agent` | backpressure, tenant quotas | on-prem edge node |
| `cloud-analytics` | batch queue | centralized analytics |

## Dependencies

- `core/contracts`, `core/core-bus`, `core/plugin-host` (required)
- `core/hal-rs` (edge agent), `core/math-kernel`, `plugins/ai-infer` (recommended)

**Must not** depend on other `domains/*`.

## Status

| Component | Status |
|-----------|--------|
| manifest + docs | ✅ scaffold |
| plugins | 🔲 planned |

## Switch domain

[`sfi.code-workspace`](../../sfi.code-workspace) · [Domains table](../../README.md#domains)
