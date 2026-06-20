# core/hal-rs

Zig hardware abstraction — **Phase 1**: synthetic capture + POSIX shared memory + HAL IPC.

## Build (requires Zig 0.13+)

```bash
cd core/hal-rs
zig build
```

Binary: `zig-out/bin/sfi-capture`

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
./zig-out/bin/sfi-capture
```

Check stats:

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/stats
```

## Modules

| Module | Role |
|--------|------|
| `frame_pool.zig` | POSIX shm pool (`/sfi.pool.N`) |
| `synthetic.zig` | gray8 test pattern |
| `ipc.zig` | `hal_ipc.h` notify + Unix socket client |

IPC spec: [`../contracts/hal_ipc.md`](../contracts/hal_ipc.md)

Part of [sfi-platform](https://github.com/StructForIndustry/sfi-platform)
