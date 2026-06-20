# Plugin wire protocol v1 (Phase 2)

JSON request/response over Unix stream for **out-of-process** plugins (Julia, etc.).  
Rust `plugin-host` converts to/from Cap'n Proto `Task` / `Result`.

## Transport

- Plugin **listens** on Unix socket (default `$XDG_RUNTIME_DIR/sfi-plugin-vision.sock`)
- Host connects per request (Phase 2) or reuses connection (future)
- Framing: `u32_le byte_len` + UTF-8 JSON body

## Request (`TaskRequest`)

```json
{
  "api_version": 1,
  "task_id": 42,
  "task_type": "vision.detect.defect",
  "frame": {
    "frame_id": 100,
    "width": 640,
    "height": 480,
    "stride": 640,
    "format": "gray8",
    "shm_name": "/sfi.pool.0",
    "byte_length": 307200,
    "offset": 0
  },
  "params": { "threshold": 128 }
}
```

Pixels are read from POSIX `shm_name` (same as HAL Phase 1).

## Response (`TaskResponse`)

```json
{
  "task_id": 42,
  "status": "ok",
  "message": "",
  "detections": [
    {
      "class_id": 1,
      "label": "bright-region",
      "score": 0.95,
      "bbox": { "x": 0.1, "y": 0.2, "width": 0.3, "height": 0.4 }
    }
  ],
  "metrics": [
    { "name": "bright_pixels", "value": 1234.0, "unit": "count" }
  ]
}
```

`status`: `ok` | `error` | `partial`

## Cap'n Proto mapping

| Wire | Cap'n Proto |
|------|----------------|
| `task_id` | `Task.id` / `Result.taskId` |
| `task_type` | `Task.type` |
| `detections[]` | `Result.payload.detections` |
| `metrics[]` | `Result.payload.metrics` |
