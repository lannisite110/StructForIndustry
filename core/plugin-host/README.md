# core/plugin-host

Rust plugin runtime (**plugin-host**) — apiVersion 0.

## Features

- `InProcessPlugin::load` — `dlopen` + `sfi_init` / `sfi_process_task` / `sfi_shutdown`
- API version check against `SFI_API_VERSION_MAJOR`

## Test

```bash
cargo test -p sfi-plugin-host --workspace
```

Part of [sfi-platform](https://github.com/StructForIndustry/sfi-platform) · Layer: Core
