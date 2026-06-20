# sfi-platform

**StructForIndustry** heterogeneous compute platform — Zig (HAL) + Rust (core bus) + Julia (math kernel) + Mojo (AI plugins).

Monorepo layout: one stable **core** framework and six **domain packs** that plug in without depending on each other.

## Quick start

```bash
git clone https://github.com/StructForIndustry/sfi-platform.git
cd sfi-platform
```

Open the multi-root workspace in VS Code / Cursor:

```bash
cursor sfi.code-workspace   # or: code sfi.code-workspace
```

Parallel work on multiple domains (optional):

```bash
git worktree add ../sfi-industrial -b feat/industrial
git worktree add ../sfi-cloud-edge -b feat/cloud-edge
```

## Architecture

```
┌─────────────────────────────────────────┐
│  Domain packs (domains/*)               │  business logic, compliance, profiles
├─────────────────────────────────────────┤
│  Tech plugins (plugins/*)               │  ai-infer, vision-2d, …
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

## Domains

| Domain | Path | Repo (if split later) | Status |
|--------|------|------------------------|--------|
| Industrial inspection | [domains/industrial-inspection](domains/industrial-inspection) | `sfi-domain-industrial` | **active** |
| Autonomous driving & robotics | [domains/autonomous-robotics](domains/autonomous-robotics) | `sfi-domain-robotics` | scaffold |
| Quantitative finance | [domains/quant-finance](domains/quant-finance) | `sfi-domain-quant` | scaffold |
| Bioinformatics & medical imaging | [domains/bio-medical](domains/bio-medical) | `sfi-domain-biomed` | scaffold |
| Aerospace & defense | [domains/aerospace-defense](domains/aerospace-defense) | `sfi-domain-aerospace` | scaffold |
| Cloud & edge computing | [domains/cloud-edge](domains/cloud-edge) | `sfi-domain-cloud-edge` | scaffold |

Domain packs may depend on `core/*` and `plugins/*` only — **never** on other domains.

## Tech plugins

| Plugin | Path | Description |
|--------|------|-------------|
| AI inference | [plugins/ai-infer](plugins/ai-infer) | Mojo — GPU/NPU model serving |
| 2D vision | [plugins/vision-2d](plugins/vision-2d) | detection, measurement, SPC hooks |

## Tools

| Tool | Path |
|------|------|
| CLI & scaffolding | [tools/sfi-cli](tools/sfi-cli) |
| Contract fake plugin | [tools/fake-plugin](tools/fake-plugin) |

## Rust workspace

```bash
cargo test --workspace
cargo run -p sfi-core-bus --bin sfi-bus   # Phase 1 bus daemon
SFI_SCHEDULER=1 cargo run -p sfi-core-bus --bin sfi-bus   # Phase 2: auto vision tasks
cargo run -p sfi-cli -- domain list
./tools/scripts/phase2-smoke.sh
```

Crates: `sfi-contracts`, `sfi-abi`, `sfi-core-bus`, `sfi-plugin-host`, `fake-plugin`, `sfi-cli`.

Zig HAL (Phase 1): `cd core/hal-rs && zig build`

## Branch naming

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feat/<domain>/<topic>` | `feat/industrial/vision-2d` |
| Contract | `contract/v<apiVersion>` | `contract/v1` |

## License

See [LICENSE](LICENSE).

## Roadmap

Phased development plan (estimates, milestones, per-domain pacing): [docs/PHASES.md](docs/PHASES.md)

## Links

- Organization: https://github.com/StructForIndustry
- This repository: https://github.com/StructForIndustry/sfi-platform
