# sfi-platform contract root — import this file to access all v0 types.

@0xf6ad917c7f4e4bd8;

using Common = import "common.capnp";
using Buffer = import "buffer.capnp";
using Frame = import "frame.capnp";
using Task = import "task.capnp";
using Result = import "result.capnp";
using Manifest = import "manifest.capnp";
using Bus = import "bus.capnp";

# Re-export apiVersion constant for code generators.
const apiVersionMajor :UInt16 = 0;
const apiVersionMinor :UInt16 = 0;
