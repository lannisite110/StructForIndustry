const std = @import("std");

pub const Config = struct {
    width: u32,
    height: u32,
    stride: u32,
};

pub fn fill_gray8(buffer: []u8, cfg: Config, frame_index: u64) void {
    std.debug.assert(buffer.len >= @as(usize, cfg.stride) * @as(usize, cfg.height));
    var y: u32 = 0;
    while (y < cfg.height) : (y += 1) {
        const row = buffer[@as(usize, y) * @as(usize, cfg.stride) ..][0..cfg.width];
        var x: u32 = 0;
        while (x < cfg.width) : (x += 1) {
            const wave: u8 = @truncate((x + y + frame_index) % 256);
            row[x] = wave;
        }
    }
}
