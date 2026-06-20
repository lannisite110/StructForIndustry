const std = @import("std");
const posix = std.posix;
const net = std.net;

const c = @cImport({
    @cInclude("hal_ipc.h");
});

pub const Notify = c.sfi_hal_frame_notify;
pub const NOTIFY_SIZE: usize = @sizeOf(c.sfi_hal_frame_notify);

pub fn build_notify(
    out: *c.sfi_hal_frame_notify,
    frame_id: u64,
    timestamp_ns: u64,
    sequence: u64,
    width: u32,
    height: u32,
    stride: u32,
    source_id: []const u8,
    pool_id: []const u8,
    slot_index: u32,
    generation: u32,
    byte_length: u64,
    shm_name: []const u8,
) void {
    out.* = std.mem.zeroes(c.sfi_hal_frame_notify);
    out.magic = c.SFI_HAL_IPC_MAGIC;
    out.version = c.SFI_HAL_IPC_VERSION;
    out.frame_id = frame_id;
    out.timestamp_ns = timestamp_ns;
    out.sequence = sequence;
    out.width = width;
    out.height = height;
    out.stride = stride;
    out.format = c.SFI_HAL_PIXEL_GRAY8;
    out.slot_index = slot_index;
    out.generation = generation;
    out.byte_length = byte_length;
    copy_cstr(&out.source_id, source_id);
    copy_cstr(&out.pool_id, pool_id);
    copy_cstr(&out.shm_name, shm_name);
}

fn copy_cstr(dst: anytype, src: []const u8) void {
    const len = @min(src.len, dst.len - 1);
    @memset(dst, 0);
    @memcpy(dst[0..len], src[0..len]);
}

pub const Client = struct {
    stream: posix.fd_t,

    pub fn connect(path: []const u8) !Client {
        const addr = try net.Address.initUnix(path);
        const stream = try posix.socket(addr.any.family, .stream, 0);
        errdefer posix.close(stream);
        try posix.connect(stream, &addr.any, addr.getOsSockLen());
        return .{ .stream = stream };
    }

    pub fn close(self: *Client) void {
        posix.close(self.stream);
        self.stream = -1;
    }

    pub fn send_notify(self: *Client, notify: *const c.sfi_hal_frame_notify) !void {
        var len_buf: [4]u8 = undefined;
        std.mem.writeInt(u32, &len_buf, @intCast(NOTIFY_SIZE), .little);
        try writeAll(self.stream, &len_buf);
        const bytes: [*]const u8 = @ptrCast(notify);
        try writeAll(self.stream, bytes[0..NOTIFY_SIZE]);
    }
};

fn writeAll(fd: posix.fd_t, bytes: []const u8) !void {
    var index: usize = 0;
    while (index < bytes.len) {
        const written = try posix.write(fd, bytes[index..]);
        if (written == 0) return error.ShortWrite;
        index += written;
    }
}
