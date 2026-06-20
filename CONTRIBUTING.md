# Contributing to sfi-platform

Repository: https://github.com/StructForIndustry/sfi-platform

## Layout rules

1. **Core** (`core/*`) — stable contracts and runtime. Changes require `apiVersion` review.
2. **Domain** (`domains/industrial-inspection`) — the sole domain pack.
3. **Plugins** (`plugins/*`) — technical modules (ai-infer, vision-2d).
4. **Tools** (`tools/*`) — CLI, generators, integration harness.

> **Scope is frozen to a single domain: industrial inspection.** Do not add new
> `domains/*` packs. New industrial work goes under `domains/industrial-inspection`.

## Branch naming

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feat/<topic>` | `feat/mindvision-capture` |
| Core | `feat/core/<topic>` | `feat/core/contracts-v1` |
| Fix | `fix/<topic>` | `fix/bus-deadlock` |
| Contract | `contract/v<N>` | `contract/v1` |

## Domain directory (fixed)

| Domain | Directory |
|--------|-----------|
| Industrial inspection | `domains/industrial-inspection` |

## Pull requests

- Link to the domain or core README you touched.
- If changing `core/contracts`, document `apiVersion` impact.
- Keep PRs scoped to one layer (core / plugin / domain) when possible.

## Workspace

Use `sfi.code-workspace` for multi-root navigation between core and domains.
