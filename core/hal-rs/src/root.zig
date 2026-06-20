pub const frame_pool = @import("frame_pool.zig");
pub const synthetic = @import("synthetic.zig");
pub const ipc = @import("ipc.zig");

pub const FramePool = frame_pool.FramePool;
pub const SyntheticConfig = synthetic.Config;
pub const fill_gray8 = synthetic.fill_gray8;
pub const IpcClient = ipc.Client;
pub const build_notify = ipc.build_notify;
pub const Notify = ipc.Notify;
