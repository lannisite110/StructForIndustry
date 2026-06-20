const std = @import("std");
const hal = @import("hal");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    var path_buf: [256]u8 = undefined;
    const bus_path = resolve_bus_socket(&path_buf);

    const frame_limit: u64 = parse_frame_limit() orelse 300;
    const fps: u64 = 30;
    const frame_ns: u64 = 1_000_000_000 / fps;

    var pool = try hal.FramePool.init(allocator, .{
        .pool_id = "hal.default",
        .slot_count = 2,
        .width = 640,
        .height = 480,
    });
    defer pool.deinit();

    var client = try hal.IpcClient.connect(bus_path);
    defer client.close();

    std.debug.print("sfi-capture: connected to {s}, sending {d} frames\n", .{ bus_path, frame_limit });

    var frame_id: u64 = 0;
    while (frame_id < frame_limit) : (frame_id += 1) {
        const slot_index: u32 = @intCast(frame_id % 2);
        const slot = pool.slot(slot_index);
        hal.fill_gray8(slot.mapped, .{
            .width = pool.width(),
            .height = pool.height(),
            .stride = pool.stride(),
        }, frame_id);

        var notify: hal.Notify = undefined;
        const shm_name = std.mem.sliceTo(&slot.shm_name, 0);
        hal.build_notify(
            &notify,
            frame_id + 1,
            monotonic_ns(),
            frame_id,
            pool.width(),
            pool.height(),
            pool.stride(),
            "synthetic-0",
            pool.pool_id(),
            slot_index,
            slot.generation,
            @intCast(slot.byte_length),
            shm_name,
        );
        try client.send_notify(&notify);

        if (frame_id + 1 < frame_limit) {
            std.Thread.sleep(frame_ns);
        }
    }
}

fn resolve_bus_socket(path_buf: []u8) [:0]const u8 {
    if (std.posix.getenv("SFI_BUS_SOCKET")) |configured| {
        return configured;
    }
    if (std.posix.getenv("XDG_RUNTIME_DIR")) |runtime| {
        return std.fmt.bufPrintZ(path_buf, "{s}/sfi-bus.sock", .{runtime}) catch "/tmp/sfi-bus.sock";
    }
    return "/tmp/sfi-bus.sock";
}

fn parse_frame_limit() ?u64 {
    const arg = std.posix.getenv("SFI_CAPTURE_FRAMES") orelse return null;
    return std.fmt.parseInt(u64, arg, 10) catch null;
}

fn monotonic_ns() u64 {
    return @as(u64, @intCast(std.time.nanoTimestamp()));
}
