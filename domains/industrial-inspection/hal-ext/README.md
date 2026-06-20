# hal-ext — industrial HAL extensions

Phase 3 stack for AOI line: simulation + **real V4L2 USB capture**.

## Components

| Binary | Role |
|--------|------|
| `sfi-line-frame` (lib) | Gray8 shm fill, YUYV→gray8, HAL notify builder |
| `sfi-line-publisher` | Periodic triggered frames (simulates encoder/PLC loop) |
| `sfi-plc-trigger` | Unix socket `TRIG` → synthetic frame (simulates PLC pulse) |
| **`sfi-v4l2-capture`** | **Real USB camera (V4L2) → Gray8 shm + HAL notify** |
| **`sfi-gige-capture`** | **GigE / GenICam scaffold (mock + SDK hook)** |
| **`sfi-modbus-plc-trigger`** | **Modbus TCP coil rising edge → HAL frame** |

## V4L2 capture (real hardware)

Linux only. Opens `$SFI_V4L2_DEVICE` (default `/dev/video0`), negotiates `GREY` or `YUYV`, publishes to `$SFI_BUS_SOCKET`.

| Env | Default | Meaning |
|-----|---------|---------|
| `SFI_V4L2_DEVICE` | `/dev/video0` | V4L2 device node |
| `SFI_V4L2_WIDTH` / `HEIGHT` | `640` / `480` | Requested resolution |
| `SFI_V4L2_MODE` | `freerun` | `freerun` or `trigger` (PLC `TRIG` socket) |
| `SFI_V4L2_FPS` | `15` | Freerun publish rate |
| `SFI_V4L2_FRAMES` | `0` | Stop after N frames (`0` = until killed) |
| `SFI_LINE_SHM` | `sfi.v4l2.capture` | POSIX shm pool name |
| `SFI_PLC_SOCKET` | `$XDG_RUNTIME_DIR/sfi-plc.sock` | Trigger mode only |

**Freerun (lab-batch profile):**

```bash
SFI_PROFILE=domains/industrial-inspection/profiles/lab-batch.yaml \
SFI_SCHEDULER=1 cargo run -p sfi-core-bus --bin sfi-bus

# separate terminal
SFI_V4L2_DEVICE=/dev/video0 SFI_V4L2_FPS=10 \
  cargo run --manifest-path domains/industrial-inspection/hal-ext/v4l2-capture/Cargo.toml
```

**Trigger mode (line-realtime + PLC socket):**

```bash
SFI_V4L2_MODE=trigger SFI_V4L2_DEVICE=/dev/video0 \
  cargo run --manifest-path domains/industrial-inspection/hal-ext/v4l2-capture/Cargo.toml
# send TRIG to SFI_PLC_SOCKET (same protocol as sfi-plc-trigger)
```

## GigE capture (scaffold)

Mock mode (`SFI_GIGE_MOCK=1`, default) generates Gray8 patterns for CI. Real SDK plugs into `SdkGigEBackend` in `gige-capture/src/gige.rs`.

| Env | Default | Meaning |
|-----|---------|---------|
| `SFI_GIGE_DEVICE` | `192.168.1.100` | Camera IP |
| `SFI_GIGE_MOCK` | `1` | Mock backend when `1` |
| `SFI_GIGE_MODE` | `freerun` | `freerun` or `trigger` |
| `SFI_GIGE_FPS` / `FRAMES` | `10` / `0` | Freerun rate / limit |

## Modbus PLC trigger (real hardware path)

Polls Modbus TCP coil; **rising edge** publishes one HAL frame (same shm path as `sfi-plc-trigger`).

| Env | Default | Meaning |
|-----|---------|---------|
| `SFI_MODBUS_ADDR` | `127.0.0.1:502` | Modbus TCP host:port |
| `SFI_MODBUS_COIL` | `0` | Coil address |
| `SFI_MODBUS_POLL_MS` | `100` | Poll interval |
| `SFI_MODBUS_MOCK` | `0` | `1` = internal mock coil for CI |

```bash
SFI_MODBUS_ADDR=192.168.0.10:502 SFI_MODBUS_COIL=0 \
  cargo run --manifest-path domains/industrial-inspection/hal-ext/modbus-plc-trigger/Cargo.toml
```

## PLC trigger protocol

Connect to `$SFI_PLC_SOCKET` (default `$XDG_RUNTIME_DIR/sfi-plc.sock`):

```
Client → 4 bytes: TRIG
Server → 4 bytes: ACK\0
```

Then one HAL frame is published to `$SFI_BUS_SOCKET` using shm `$SFI_LINE_SHM`.

## Quick start (simulation)

```bash
# Terminal 1 — bus + scheduler
SFI_PROFILE=domains/industrial-inspection/profiles/line-realtime.yaml \
SFI_SCHEDULER=1 cargo run -p sfi-core-bus --bin sfi-bus

# Terminal 2 — Julia defect-detect
julia --project=domains/industrial-inspection/plugins/defect-detect \
  domains/industrial-inspection/plugins/defect-detect/server.jl

# Terminal 3 — PLC trigger gateway (synthetic)
cargo run --manifest-path domains/industrial-inspection/hal-ext/plc-trigger/Cargo.toml

# Terminal 4 — send trigger
python3 -c "import socket; s=socket.socket(socket.AF_UNIX); s.connect('$XDG_RUNTIME_DIR/sfi-plc.sock'); s.sendall(b'TRIG'); print(s.recv(4))"
```

## SHM naming

HAL notify uses `/sfi.aoi.line.0` style names; filesystem path is `/dev/shm/sfi.aoi.line.0`.
Julia `defect-detect` and Rust mocks resolve this automatically.

## Tests

```bash
cargo test --manifest-path domains/industrial-inspection/hal-ext/line-frame/Cargo.toml
cargo test -p sfi-core-bus shm_defect_e2e
./tools/scripts/shm-julia-e2e.sh   # requires Julia
./tools/scripts/plc-trigger-e2e.sh # PLC TRIG → bus → mock defect-detect
./tools/scripts/v4l2-capture-e2e.sh # skips if no /dev/video42 (use setup-v4l2loopback.sh)
./tools/scripts/v4l2-trigger-e2e.sh # V4L2 + TRIG socket
./tools/scripts/gige-capture-e2e.sh # mock GigE
./tools/scripts/modbus-plc-e2e.sh     # mock Modbus coil
```
