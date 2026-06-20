# Bioinformatics & Medical Imaging

Part of [sfi-platform](https://github.com/StructForIndustry/sfi-platform) · Domain: **Bioinformatics & Medical Imaging**  
Split repo (future): `StructForIndustry/sfi-domain-biomed`

## Overview

Sequencing instruments, genomic analysis pipelines, and medical imaging AI — with traceability and privacy hooks.

## Focus

| Area | Stack | Notes |
|------|-------|-------|
| Instruments | Zig `hal-ext` | sequencers, modality-specific devices |
| Pipeline orchestration | Rust `core-bus` | batch jobs, provenance, access control |
| Analysis | Julia plugins | sequence alignment, stats, imaging prep |
| Inference | Mojo `plugins/ai-infer` | diagnostic AI acceleration |

## Throughput vs latency

| Workload | Profile |
|----------|---------|
| Clinical imaging | moderate latency, strict audit |
| Genomic batch | high throughput, queue-based |

## Directory layout

```
bio-medical/
├── manifest.yaml
├── hal-ext/
├── plugins/
├── profiles/
├── compliance/         # HIPAA / FDA traceability placeholders
└── examples/
```

## Profiles

| Profile | Scheduler | Notes |
|---------|-----------|-------|
| `clinical-imaging` | bounded latency + full audit | diagnostic imaging |
| `genomic-batch` | batch queue, maximize throughput | offline pipelines |

## Dependencies

- `core/contracts`, `core/hal-rs`, `core/core-bus`, `core/plugin-host` (required)
- `core/math-kernel`, `plugins/ai-infer` (recommended)

**Must not** depend on other `domains/*`.

## Status

| Component | Status |
|-----------|--------|
| manifest + docs | ✅ scaffold |
| plugins | 🔲 planned |

## Switch domain

[`sfi.code-workspace`](../../sfi.code-workspace) · [Domains table](../../README.md#domains)
