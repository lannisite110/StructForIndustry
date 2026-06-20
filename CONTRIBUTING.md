# Contributing to sfi-platform

Repository: https://github.com/StructForIndustry/sfi-platform

## Layout rules

1. **Core** (`core/*`) — stable contracts and runtime. Changes require `apiVersion` review.
2. **Domains** (`domains/*`) — must not import or depend on other domains.
3. **Plugins** (`plugins/*`) — cross-domain technical modules only.
4. **Tools** (`tools/*`) — CLI, generators, integration harness.

## Branch naming

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feat/<domain>/<topic>` | `feat/industrial/vision-2d` |
| Core | `feat/core/<topic>` | `feat/core/contracts-v1` |
| Fix | `fix/<topic>` | `fix/bus-deadlock` |
| Contract | `contract/v<N>` | `contract/v1` |

## Domain directory names (fixed)

| Domain | Directory |
|--------|-----------|
| Industrial inspection | `domains/industrial-inspection` |
| Autonomous & robotics | `domains/autonomous-robotics` |
| Quant finance | `domains/quant-finance` |
| Bio & medical | `domains/bio-medical` |
| Aerospace & defense | `domains/aerospace-defense` |
| Cloud & edge | `domains/cloud-edge` |

## Pull requests

- Link to the domain README you touched.
- If changing `core/contracts`, document `apiVersion` impact.
- Keep PRs scoped to one domain or core layer when possible.

## Workspace

Use `sfi.code-workspace` for multi-root navigation between core and domains.
