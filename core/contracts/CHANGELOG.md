# Contract changelog

All notable changes to `core/contracts` follow [apiVersion](VERSION).

## apiVersion 0 (draft)

Initial draft schemas:

- `Frame`, `FrameRef`, `PixelFormat`
- `Task`, `TaskInput`, common task parameter envelope
- `Result`, `ResultStatus`, `DetectionList`, `InferOutput`
- `PluginManifest`, `ResourceReq`
- `SharedBuffer`, `BufferHandle` (POSIX shm / fd passing)
- Bus envelopes: `FrameEvent`, `TaskDispatch`, `ResultEvent`, `PluginHealth`

**Stability**: apiVersion 0 permits backward-compatible field additions. Breaking changes
may occur until apiVersion 1 is declared stable.
