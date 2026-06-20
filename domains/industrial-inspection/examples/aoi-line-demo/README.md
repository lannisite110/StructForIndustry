# AOI line demo (Phase 3)

One-command smoke for industrial-inspection stack:

```bash
chmod +x start.sh
./start.sh
```

Components exercised:

| Process | Role |
|---------|------|
| `sfi-mock-vision` | vision-2d stand-in (plugin wire v1) |
| `sfi-bus` | HAL listener + scheduler + HTTP `/stats` |
| `sfi-mes-reporter` | REST stub `POST /inspection/result` |

Profile: [`profiles/line-realtime.yaml`](../../profiles/line-realtime.yaml)

Full Julia path (requires shm frame):

```bash
SFI_SCHEDULER=1 SFI_SUPERVISE_VISION=1 cargo run -p sfi-core-bus --bin sfi-bus
julia --project=plugins/vision-2d plugins/vision-2d/server.jl
```
