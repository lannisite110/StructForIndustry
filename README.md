# StructForIndustry

**Industrial AOI edge-inspection platform** — Zig (HAL) + Rust (core bus) + Julia (math kernel) + ONNX/`ort` (AI inference).

A focused, single-domain platform: capture an industrial line frame, run defect
detection (classical CV or ONNX), publish OK/NG with full traceability — under a
tight latency budget on edge hardware.

> Scope is deliberately **one domain: industrial inspection**. The multi-domain
> scaffolding has been removed to keep the platform converged on a single,
> demonstrable product. See [docs/PHASES.md](docs/PHASES.md).

## Quick start

```bash
# Rust workspace
cargo test --workspace
SFI_SCHEDULER=1 cargo run -p sfi-core-bus --bin sfi-bus   # bus + auto vision tasks

# Local CI mirror (fmt, clippy, tests, E2E, bench)
./tools/scripts/local-contracts-smoke.sh
```

Open the multi-root workspace in VS Code / Cursor:

```bash
cursor sfi.code-workspace
```

## Architecture

```
┌─────────────────────────────────────────┐
│  Domain pack: industrial-inspection     │  AOI line logic, profiles, compliance
├─────────────────────────────────────────┤
│  Tech plugins (plugins/*)               │  ai-infer (ONNX), vision-2d
├─────────────────────────────────────────┤
│  Core framework (core/*)                │  contracts, HAL, bus, math, plugin-host
└─────────────────────────────────────────┘
```

| Layer | Path | Languages |
|-------|------|-----------|
| Contracts | [`core/contracts`](core/contracts) | Cap'n Proto, C ABI |
| HAL | [`core/hal-rs`](core/hal-rs) | Zig |
| Core bus | [`core/core-bus`](core/core-bus) | Rust |
| Math kernel | [`core/math-kernel`](core/math-kernel) | Julia |
| Plugin host | [`core/plugin-host`](core/plugin-host) | Rust |

## Domain

| Domain | Path | Status |
|--------|------|--------|
| Industrial inspection | [domains/industrial-inspection](domains/industrial-inspection) | **active (sole domain)** |

The industrial pack contains the AOI line demo, defect-detect / SPC Julia plugins,
HAL extensions (V4L2, GigE scaffold, MindVision, Modbus/OPC-UA PLC triggers), and
line/lab profiles. See [its README](domains/industrial-inspection/README.md) and
[hal-ext README](domains/industrial-inspection/hal-ext/README.md).

## Tech plugins

| Plugin | Path | Description |
|--------|------|-------------|
| AI inference | [plugins/ai-infer](plugins/ai-infer) | ONNX via Rust `ort` (reference stub default) |
| 2D vision | [plugins/vision-2d](plugins/vision-2d) | detection, measurement, SPC hooks |

## Tools

| Tool | Path |
|------|------|
| CLI & scaffolding | [tools/sfi-cli](tools/sfi-cli) |
| Contract fake plugin | [tools/fake-plugin](tools/fake-plugin) |
| Scripts (E2E, bench, CI mirror) | [tools/scripts](tools/scripts) |

## Rust workspace

```bash
cargo test --workspace
cargo run -p sfi-core-bus --bin sfi-bus                    # bus daemon
SFI_SCHEDULER=1 cargo run -p sfi-core-bus --bin sfi-bus    # auto vision tasks
cargo run -p sfi-cli -- domain list
```

Crates: `sfi-contracts`, `sfi-abi`, `sfi-core-bus`, `sfi-plugin-host`, `fake-plugin`, `sfi-cli`.

Zig HAL: `cd core/hal-rs && zig build`

## Branch naming

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feat/<topic>` | `feat/mindvision-capture` |
| Contract | `contract/v<apiVersion>` | `contract/v1` |
| Fix | `fix/<topic>` | `fix/bus-deadlock` |

## License

See [LICENSE](LICENSE).

## Roadmap

Phased development plan: [docs/PHASES.md](docs/PHASES.md)
