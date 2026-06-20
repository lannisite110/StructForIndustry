#!/usr/bin/env bash
# Load v4l2loopback virtual camera for CI / local E2E (Linux only).
set -euo pipefail

VIDEO_NR="${SFI_V4L2_LOOPBACK_NR:-42}"
DEVICE="/dev/video${VIDEO_NR}"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "v4l2loopback: skip (not Linux)"
  exit 0
fi

if [[ -e "$DEVICE" ]]; then
  echo "v4l2loopback: $DEVICE already present"
  exit 0
fi

if ! command -v modprobe >/dev/null 2>&1; then
  echo "v4l2loopback: modprobe missing" >&2
  exit 1
fi

if ! lsmod | grep -q '^v4l2loopback'; then
  if ! sudo modprobe v4l2loopback devices=1 "video_nr=${VIDEO_NR}" exclusive_caps=1 max_buffers=4 2>/dev/null; then
    echo "v4l2loopback: modprobe failed (need sudo / kernel module)" >&2
    exit 0
  fi
fi

if [[ ! -e "$DEVICE" ]]; then
  echo "v4l2loopback: failed to create $DEVICE (skipping)" >&2
  exit 0
fi

echo "v4l2loopback: ready at $DEVICE"

# Optional test pattern feed (YUYV) so captures are non-empty.
if [[ "${SFI_V4L2_FEED:-1}" == "1" ]] && command -v ffmpeg >/dev/null; then
  if ! pgrep -f "ffmpeg.*${DEVICE}" >/dev/null 2>&1; then
    ffmpeg -hide_banner -loglevel error \
      -f lavfi -i "testsrc=size=320x240:rate=10" \
      -pix_fmt yuyv422 -f v4l2 "$DEVICE" &
    echo "v4l2loopback: ffmpeg test pattern → $DEVICE (pid $!)"
    sleep 0.5
  fi
fi
