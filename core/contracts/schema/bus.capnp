# Bus envelope messages — topics used by core-bus pub/sub.

@0xe59c806b6e5d3ac7;

using Common = import "common.capnp";
using Frame = import "frame.capnp";
using Task = import "task.capnp";
using Result = import "result.capnp";
using Manifest = import "manifest.capnp";

# Topic: "frame.new"
struct FrameEvent {
  api @0 :Common.ApiVersion;
  frame @1 :Frame.Frame;
  publishedAtNs @2 :UInt64;
}

# Generic envelope for logging / future multiplexed streams.
struct BusEnvelope {
  topic @0 :Text;
  publishedAtNs @1 :UInt64;
  api @2 :Common.ApiVersion;

  body @3 :Body;

  struct Body {
    union {
      frame @0 :FrameEvent;
      task @1 :Task.TaskDispatch;
      result @2 :Result.ResultEvent;
      health @3 :Manifest.PluginHealthEvent;
    }
  }
}
