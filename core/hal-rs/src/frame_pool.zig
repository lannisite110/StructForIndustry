const std = @import("std");
const posix = std.posix;

pub const PoolConfig = struct {
    pool_id: []const u8 = "hal.default",
    slot_count: u32 = 4,
    width: u32 = 640,
    height: u32 = 480,
};

pub const Slot = struct {
    shm_name: [32]u8,
    generation: u32,
    byte_length: usize,
    fd: posix.fd_t,
    mapped: []u8,
};

pub const FramePool = struct {
    allocator: std.mem.Allocator,
    config: PoolConfig,
    slots: []Slot,

    pub fn init(allocator: std.mem.Allocator, config: PoolConfig) !FramePool {
        const slots = try allocator.alloc(Slot, config.slot_count);
        errdefer allocator.free(slots);
        @memset(slots, undefined);

        var pool = FramePool{
            .allocator = allocator,
            .config = config,
            .slots = slots,
        };
        errdefer pool.deinit();

        var i: u32 = 0;
        while (i < config.slot_count) : (i += 1) {
            try pool.init_slot(i);
        }
        return pool;
    }

    pub fn deinit(self: *FramePool) void {
        for (self.slots) |*slot| {
            if (slot.mapped.len > 0) {
                posix.munmap(slot.mapped);
            }
            if (slot.fd >= 0) {
                posix.close(slot.fd);
                const name = std.mem.sliceTo(&slot.shm_name, 0);
                if (name.len > 0) {
                    posix.shm_unlink(name) catch {};
                }
            }
        }
        self.allocator.free(self.slots);
    }

    fn init_slot(self: *FramePool, index: u32) !void {
        const stride = self.config.width;
        const byte_length = @as(usize, stride) * @as(usize, self.config.height);
        var name_buf: [32]u8 = undefined;
        const shm_name = try std.fmt.bufPrintZ(&name_buf, "/sfi.pool.{d}", .{index});

        const fd = try posix.shm_open(shm_name, .{ .AC = true, .CREAT = true, .TRUNC = true }, 0o600);
        errdefer posix.close(fd);
        try posix.ftruncate(fd, byte_length);
        const mapped = try posix.mmap(null, byte_length, .{ .TYPE = .SHARED, .READ = true, .WRITE = true }, .{ .SHARED = true }, fd, 0);
        errdefer posix.munmap(mapped);

        var slot = Slot{
            .shm_name = undefined,
            .generation = 1,
            .byte_length = byte_length,
            .fd = fd,
            .mapped = mapped,
        };
        @memset(&slot.shm_name, 0);
        @memcpy(slot.shm_name[0..shm_name.len], shm_name);
        self.slots[index] = slot;
    }

    pub fn slot(self: *FramePool, index: u32) *Slot {
        return &self.slots[index];
    }

    pub fn width(self: *const FramePool) u32 {
        return self.config.width;
    }

    pub fn height(self: *const FramePool) u32 {
        return self.config.height;
    }

    pub fn stride(self: *const FramePool) u32 {
        return self.config.width;
    }

    pub fn pool_id(self: *const FramePool) []const u8 {
        return self.config.pool_id;
    }
};
