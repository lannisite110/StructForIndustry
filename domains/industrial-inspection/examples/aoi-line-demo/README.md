# AOI line demo (Phase 3)

One-command smoke for industrial-inspection stack.

## Local script

```bash
chmod +x start.sh docker-entrypoint.sh
./start.sh
```

Open http://127.0.0.1:18080/ for AOI preview (SPC trend + NG list + frame archive paths).

## Docker Compose (bus + Prometheus)

```bash
docker compose up --build
```

| URL | Purpose |
|-----|---------|
| http://localhost:8080/ | AOI dashboard |
| http://localhost:8080/metrics | Prometheus text |
| http://localhost:9090/ | Prometheus UI |

Frame archives land in Docker volume `sfi-data` under `frames/`.

## Profiles

| Profile | Plugin |
|---------|--------|
| `line-realtime.yaml` | defect-detect (default) |
| `line-infer.yaml` | ai-infer (`infer.onnx`) |
| `lab-batch.yaml` | batch queue, no drop |

Switch infer profile:

```bash
SFI_PROFILE=domains/industrial-inspection/profiles/line-infer.yaml \
SFI_INFER_SOCKET=/tmp/sfi-infer.sock \
cargo run -p sfi-core-bus --bin sfi-bus
# separate terminal:
cargo run --manifest-path plugins/ai-infer/Cargo.toml
```

## Components

| Process | Role |
|---------|------|
| `sfi-mock-defect-detect` | plugin wire v1 + ROI + shm |
| `sfi-bus` | HAL + scheduler + audit + frame archive |
| `sfi-mes-reporter` | REST stub |
| `sfi-line-publisher` | triggered shm frames |

Recording a demo GIF: run `./start.sh`, capture http://127.0.0.1:18080/ with Peek or OBS.
