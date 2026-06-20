#!/usr/bin/env bash
set -euo pipefail
RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/sfi-docker-demo"
mkdir -p "$RUN_DIR" "$SFI_DATA_DIR/frames"
BUS_SOCK="$RUN_DIR/bus.sock"
VISION_SOCK="$RUN_DIR/vision.sock"
HTTP_ADDR="${SFI_BUS_HTTP:-0.0.0.0:8080}"
MES_ADDR="127.0.0.1:8090"
PROFILE="/app/domains/industrial-inspection/profiles/line-realtime.yaml"
SHM_NAME="sfi.aoi.docker"

SFI_VISION_PLUGIN_SOCKET="$VISION_SOCK" sfi-mock-defect-detect &
SFI_MES_ADDR="$MES_ADDR" sfi-mes-reporter &
sleep 0.5

SFI_BUS_SOCKET="$BUS_SOCK" \
SFI_BUS_HTTP="$HTTP_ADDR" \
SFI_VISION_PLUGIN_SOCKET="$VISION_SOCK" \
SFI_PROFILE="$PROFILE" \
SFI_MES_ENABLED=1 \
SFI_MES_ENDPOINT="http://$MES_ADDR/inspection/result" \
SFI_SCHEDULER=1 \
sfi-bus &
sleep 1

if command -v sfi-line-publisher >/dev/null; then
  SFI_BUS_SOCKET="$BUS_SOCK" SFI_LINE_SHM="$SHM_NAME" SFI_LINE_FRAMES=3 SFI_LINE_INTERVAL_MS=300 \
    sfi-line-publisher &
fi

echo "AOI demo listening on http://${HTTP_ADDR#*://}/"
exec tail -f /dev/null
