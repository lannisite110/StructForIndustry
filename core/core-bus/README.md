# core/core-bus

Rust **core-bus** daemon — Phase 1 MVP.

## Run

```bash
cargo run -p sfi-core-bus --bin sfi-bus
```

Environment:

| Variable | Default |
|----------|---------|
| `SFI_BUS_SOCKET` | `$XDG_RUNTIME_DIR/sfi-bus.sock` or `/tmp/sfi-bus.sock` |
| `SFI_BUS_HTTP` | `127.0.0.1:8080` |

## HTTP

| Path | Description |
|------|-------------|
| `GET /health` | liveness + apiVersion |
| `GET /stats` | `frames_received`, `last_frame_id`, `last_timestamp_ns` |

## HAL ingress

Listens on Unix socket for [`hal_ipc`](../contracts/hal_ipc.md) framed notifications, publishes internal `frame.new` (`bus.FrameEvent` Cap'n Proto bytes).

## Test

```bash
cargo test -p sfi-core-bus
```

Part of [sfi-platform](https://github.com/StructForIndustry/sfi-platform)
