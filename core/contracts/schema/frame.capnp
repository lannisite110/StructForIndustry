# Frame — atomic visual/sensor raster unit for vision and imaging pipelines.

@0xa15e4c3d2a1f9683;

using Common = import "common.capnp";
using Buffer = import "buffer.capnp";

enum PixelFormat {
  unknown @0;
  gray8 @1;
  gray16 @2;
  rgb8 @3;
  bgr8 @4;
  rgba8 @5;
  bgra8 @6;
  yuv420 @7;
  yuv422 @8;
  depth16 @9;    # uint16 depth mm or raw device units (see metadata)
  depth32f @10;  # float32 meters
}

struct FrameRef {
  # Stable frame id assigned by HAL / core-bus.
  id @0 :UInt64;

  # Optional: pass buffer if consumer is not subscribed to frame cache.
  buffer @1 :Buffer.SharedBuffer;

  sourceId @2 :Text;
  timestampNs @3 :UInt64;
}

struct Frame {
  id @0 :UInt64;
  timestampNs @1 :UInt64;

  # Logical source (camera, lidar projection, file replay, ...).
  sourceId @2 :Text;

  width @3 :UInt32;
  height @4 :UInt32;
  format @5 :PixelFormat;
  stride @6 :UInt32;  # bytes per row (may exceed width * bpp)

  # Pixel payload — always via shared memory in production hot paths.
  buffer @7 :Buffer.SharedBuffer;

  # Optional sequence / trigger metadata from HAL.
  sequence @8 :UInt64;
  triggerId @9 :UInt64;

  extensions @10 :Common.Extensions;
}
