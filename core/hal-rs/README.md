# core/hal-rs

Synthetic hardware abstraction (**Phase 1**), in **Rust**: Gray8 test pattern +
POSIX shared memory + HAL IPC publisher. Builds `sfi-capture`, the minimal
"camera stand-in" that feeds core-bus.

> Previously a Zig prototype; reimplemented in Rust so the whole edge stack is a
> single `cargo` build/test. Same `HalFrameNotify` wire format as the production
> line publishers.

## Build

```bash
cargo build -p sfi-hal-capture --bin sfi-capture
```

## Run with core-bus

Terminal 1:

```bash
export SFI_BUS_SOCKET=/tmp/sfi-bus.sock
export SFI_BUS_HTTP=127.0.0.1:8080
cargo run -p sfi-core-bus --bin sfi-bus
```

Terminal 2:

```bash
export SFI_BUS_SOCKET=/tmp/sfi-bus.sock
export SFI_CAPTURE_FRAMES=100
cargo run -p sfi-hal-capture --bin sfi-capture
```

Check stats:

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/stats
```

## Env

| Variable | Default | Meaning |
|----------|---------|---------|
| `SFI_BUS_SOCKET` | `$XDG_RUNTIME_DIR/sfi-bus.sock` | HAL Unix socket |
| `SFI_CAPTURE_FRAMES` | `300` | Frames to publish |
| `SFI_CAPTURE_FPS` | `30` | Publish rate |

## Modules

| Item | Role |
|------|------|
| `lib.rs` `build_notify` | Build `HalFrameNotify` for a synthetic frame |
| `main.rs` `sfi-capture` | Fill shm test pattern + publish frames |

Shm fill / read uses `sfi-plugin-host::shm_gray8`. IPC spec:
[`../contracts/hal_ipc.md`](../contracts/hal_ipc.md)

Quick check: `tools/scripts/phase1-smoke.sh`.

Part of [sfi-platform](https://github.com/lannisite110/StructForIndustry).
