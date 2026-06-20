# hal-ext — industrial HAL extensions

Phase 3 simulation stack for AOI line without real PLC/GigE hardware.

## Components

| Binary | Role |
|--------|------|
| `sfi-line-frame` (lib) | Gray8 shm fill + HAL notify builder |
| `sfi-line-publisher` | Periodic triggered frames (simulates encoder/PLC loop) |
| `sfi-plc-trigger` | Unix socket `TRIG` → single frame (simulates PLC pulse) |

## PLC trigger protocol

Connect to `$SFI_PLC_SOCKET` (default `$XDG_RUNTIME_DIR/sfi-plc.sock`):

```
Client → 4 bytes: TRIG
Server → 4 bytes: ACK\0
```

Then one HAL frame is published to `$SFI_BUS_SOCKET` using shm `$SFI_LINE_SHM`.

## Quick start

```bash
# Terminal 1 — bus + scheduler
SFI_PROFILE=domains/industrial-inspection/profiles/line-realtime.yaml \
SFI_SCHEDULER=1 cargo run -p sfi-core-bus --bin sfi-bus

# Terminal 2 — Julia defect-detect
julia --project=domains/industrial-inspection/plugins/defect-detect \
  domains/industrial-inspection/plugins/defect-detect/server.jl

# Terminal 3 — PLC trigger gateway
cargo run --manifest-path domains/industrial-inspection/hal-ext/plc-trigger/Cargo.toml

# Terminal 4 — send trigger
python3 -c "import socket; s=socket.socket(socket.AF_UNIX); s.connect('$XDG_RUNTIME_DIR/sfi-plc.sock'); s.sendall(b'TRIG'); print(s.recv(4))"
```

## SHM naming

HAL notify uses `/sfi.aoi.line.0` style names; filesystem path is `/dev/shm/sfi.aoi.line.0`.
Julia `defect-detect` and Rust mocks resolve this automatically.

## Tests

```bash
cargo test -p sfi-core-bus shm_defect_e2e
./tools/scripts/shm-julia-e2e.sh   # requires Julia
./tools/scripts/plc-trigger-e2e.sh # PLC TRIG → bus → mock defect-detect
```
