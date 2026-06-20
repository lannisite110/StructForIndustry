# HAL ↔ core-bus IPC (Phase 1)

Binary notification from **hal-rs capture** to **core-bus**. Pixel payload lives in POSIX shared memory; this message carries metadata + shm handle name only.

## Transport

- Unix domain stream socket (default: `$XDG_RUNTIME_DIR/sfi-bus.sock`, fallback `/tmp/sfi-bus.sock`)
- Framing: `u32_le message_len` + `sfi_hal_frame_notify` (144 bytes)

## C layout

See [`abi/hal_ipc.h`](abi/hal_ipc.h).

| Field | Type | Notes |
|-------|------|-------|
| magic | u32 | `0x00494653` (`"SFI\0"`) |
| version | u16 | `1` |
| frame_id | u64 | monotonic per source |
| timestamp_ns | u64 | CLOCK_MONOTONIC |
| sequence | u64 | optional trigger sequence |
| width/height/stride | u32 | pixels |
| format | u8 | `1` = gray8 |
| source_id | char[32] | e.g. `synthetic-0` |
| pool_id | char[16] | e.g. `hal.default` |
| slot_index | u32 | index in pool |
| generation | u32 | reuse generation |
| byte_length | u64 | mapped shm size |
| shm_name | char[32] | POSIX shm name, e.g. `/sfi.pool.0` |

## Shared memory

HAL creates `shm_name` with `shm_open` + `ftruncate`, writes pixels, sends notify. core-bus may `mmap` the same name for validation (optional in Phase 1).

Cap'n Proto `bus.FrameEvent` is built **inside core-bus** from this notification.
