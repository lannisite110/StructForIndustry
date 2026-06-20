# tools/fake-plugin

Mock **in-process** plugin for Phase 0 contract validation.

Implements [core/contracts/abi/sfi.h](../core/contracts/abi/sfi.h) and returns a synthetic `DetectionList` for `vision.*` tasks.

## Build

```bash
cargo build -p fake-plugin
```

Artifact: `target/debug/libfake_plugin.so` (Linux).

## Test (in-crate C ABI)

```bash
cargo test -p fake-plugin
```

## Integration

Loaded by [core/plugin-host](../core/plugin-host) integration test:

```bash
cargo test -p sfi-plugin-host --test load_fake_plugin
```

Part of [sfi-platform](https://github.com/StructForIndustry/sfi-platform)
