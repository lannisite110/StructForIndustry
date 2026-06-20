# Shared memory buffer handles — pixel payloads never inline in hot-path messages.

@0x9a4d3b2c1f0e8572;

using Common = import "common.capnp";

struct BufferHandle {
  # Logical pool identifier (e.g. "hal.default" or device id).
  poolId @0 :Text;

  # Slot index inside the pool (stable until release).
  slotIndex @1 :UInt32;

  # Total mapped byte length of the slot.
  byteLength @2 :UInt64;

  # Byte offset of payload within the mapped region.
  offset @3 :UInt64;

  # Transport-specific handle (one or more may be set).
  transport @4 :Transport;

  struct Transport {
    # POSIX shared memory object name (shm_open).
    shmName @0 :Text;

    # File descriptor for SCM_RIGHTS / dma-buf (sent out-of-band on Unix socket).
    fd @1 :Int32;

    # DMA-BUF plane index when fd refers to a dma-buf object.
    dmaBufPlane @2 :UInt8;
  }

  extensions @5 :Common.Extensions;
}

struct SharedBuffer {
  # Full descriptor: handle + generation for reuse detection.
  handle @0 :BufferHandle;
  generation @1 :UInt32;
}
