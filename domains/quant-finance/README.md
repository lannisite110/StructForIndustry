# Quantitative Finance

Part of [sfi-platform](https://github.com/StructForIndustry/sfi-platform) · Domain: **Quantitative Finance**  
Split repo (future): `StructForIndustry/sfi-domain-quant`

## Overview

Low-latency trading infrastructure, strategy execution, risk controls, and research backtesting.

## Focus

| Area | Stack | Notes |
|------|-------|-------|
| Market data | Zig `hal-ext` | NIC tuning, feed handlers, capture |
| Trading core | Rust `core-bus` | order routing, risk gates, audit |
| Research | Julia plugins | backtest, factor models, risk simulation |
| Hot paths | Rust in-process plugins | microsecond-sensitive logic |

## Latency target

| Path | Target |
|------|--------|
| Tick → decision | platform-specific, often &lt; 1 ms |
| Backtest batch | throughput-optimized |

## Directory layout

```
quant-finance/
├── manifest.yaml
├── hal-ext/
├── plugins/
├── profiles/
├── compliance/         # trade audit, surveillance hooks
└── examples/
```

## Profiles

| Profile | Scheduler | Notes |
|---------|-----------|-------|
| `hft-core` | pinned cores, no GC in hot path | production trading |
| `research-backtest` | batch queue | offline strategy research |

## Dependencies

- `core/contracts`, `core/core-bus`, `core/plugin-host` (required)
- `core/math-kernel` (recommended for research plugins)
- Zig `hal-ext` (optional, for feed capture)

**Must not** depend on other `domains/*`.

## Status

| Component | Status |
|-----------|--------|
| manifest + docs | ✅ scaffold |
| plugins | 🔲 planned |

## Switch domain

[`sfi.code-workspace`](../../sfi.code-workspace) · [Domains table](../../README.md#domains)
