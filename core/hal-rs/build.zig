const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const abi_include = b.path("../contracts/abi");

    const hal_mod = b.createModule(.{
        .root_source_file = b.path("src/root.zig"),
        .target = target,
        .optimize = optimize,
    });
    hal_mod.linkLibC();
    hal_mod.addIncludePath(abi_include);

    const capture = b.addExecutable(.{
        .name = "sfi-capture",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
            .imports = &.{
                .{ .name = "hal", .module = hal_mod },
            },
        }),
    });
    capture.linkLibC();
    capture.root_module.addIncludePath(abi_include);

    b.installArtifact(capture);

    const unit_tests = b.addTest(.{
        .root_module = hal_mod,
    });
    unit_tests.linkLibC();
    unit_tests.root_module.addIncludePath(abi_include);

    const run_unit_tests = b.addRunArtifact(unit_tests);
    const test_step = b.step("test", "Run hal-rs unit tests");
    test_step.dependOn(&run_unit_tests.step);
}
