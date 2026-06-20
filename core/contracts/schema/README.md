# Cap'n Proto schemas (apiVersion 0)

| File | Primary types |
|------|----------------|
| [common.capnp](common.capnp) | `ApiVersion`, `StatusCode`, `TimingMetrics`, `ResourceReq`, `Extensions` |
| [buffer.capnp](buffer.capnp) | `BufferHandle`, `SharedBuffer` |
| [frame.capnp](frame.capnp) | `Frame`, `FrameRef`, `PixelFormat` |
| [task.capnp](task.capnp) | `Task`, `TaskInput`, `TaskDispatch` |
| [result.capnp](result.capnp) | `Result`, `DetectionList`, `InferOutput`, `ResultEvent` |
| [manifest.capnp](manifest.capnp) | `PluginManifest`, `PluginHealth` |
| [bus.capnp](bus.capnp) | `FrameEvent`, `BusEnvelope` |
| [sfi.capnp](sfi.capnp) | Root import + version constants |

## Bus topics

| Topic | Message type |
|-------|----------------|
| `frame.new` | `bus.FrameEvent` |
| `task.dispatch` | `task.TaskDispatch` |
| `task.done` | `result.ResultEvent` |
| `plugin.health` | `manifest.PluginHealthEvent` |

## Compile / validate

Requires [Cap'n Proto](https://capnp.org/) (`capnp` compiler).

```bash
# From this directory (schema/)
capnp compile -o c++ -I . sfi.capnp

# Validate all schemas parse
for f in *.capnp; do capnp compile -o- -I . "$f"; done
```

Rust (future): `capnpc` in `core/core-bus` build.rs.

Zig / Julia: generate bindings from compiled schemas or use capnp RPC runtime.

## Design rules

1. **Pixels never inline** on hot paths — use `SharedBuffer` / `BufferHandle`.
2. **Task.type** is a capability string; matching rules live in plugin-host.
3. **Extensions** on structs allow apiVersion 0 evolution without breaking parsers.
4. **apiVersion** in envelopes (`api` field) must match `VERSION` at load time.

## C ABI

In-process plugins implement [abi/sfi.h](../abi/sfi.h).
