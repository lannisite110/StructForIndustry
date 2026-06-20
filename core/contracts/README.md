# core/contracts

Cross-language contracts for **sfi-platform** — the constitution layer.

Defines `Frame`, `Task`, `Result`, `PluginManifest`, and `apiVersion` negotiation.

| Artifact | Purpose |
|----------|---------|
| [schema/](schema/) | Cap'n Proto definitions — start with [schema/README.md](schema/README.md) |
| [rust/sfi-contracts](rust/sfi-contracts/) | Rust generated types (`capnpc`) |
| [abi/sfi.h](abi/sfi.h) | C ABI headers for in-process plugins |
| [abi/hal_ipc.h](abi/hal_ipc.h) | HAL → core-bus binary notify (Phase 1) |
| [hal_ipc.md](hal_ipc.md) | HAL IPC transport spec |
| [VERSION](VERSION) | apiVersion major (0 = draft) |
| [CHANGELOG.md](CHANGELOG.md) | Contract version history |

**Rule:** Domain packs and plugins depend on published contract versions only. Breaking changes require a new `apiVersion`.

Part of [sfi-platform](https://github.com/StructForIndustry/sfi-platform) · Layer: Core
