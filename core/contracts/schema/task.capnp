# Task — schedulable work unit dispatched to plugins.

@0xb26f5d4e3b2a0794;

using Common = import "common.capnp";
using Frame = import "frame.capnp";

enum TaskPriority {
  low @0;
  normal @1;
  high @2;
  realtime @3;
}

struct TaskParams {
  # Opaque parameters (JSON, CBOR, or domain-specific binary).
  # Prefer structured Result payloads over bloated params.
  encoding @0 :Text;  # "json", "cbor", "none"
  body @1 :Data;
}

struct TaskInput {
  union {
    none @0 :Void;
    frameRef @1 :Frame.FrameRef;
    # Full frame for tests, small images, or replay without shm pool.
    frame @2 :Frame.Frame;
    # Domain-specific serialized input (e.g. ROI params, recipe blob).
    raw @3 :Data;
  }
}

struct Task {
  id @0 :UInt64;

  # Dot-separated capability string matched against PluginManifest.capabilities.
  # Examples: "vision.detect.defect", "infer.onnx", "spc.metrics"
  type @1 :Text;

  priority @2 :TaskPriority;
  deadlineNs @3 :UInt64;  # 0 = no deadline

  # Trace correlation across bus messages and REST calls.
  correlationId @4 :Text;

  # Tenant / line / station context for multi-tenant edge deployments.
  tenantId @5 :Text;
  contextId @6 :Text;  # e.g. line id, robot id, portfolio id

  input @7 :TaskInput;
  params @8 :TaskParams;

  extensions @9 :Common.Extensions;
}

# Published on topic "task.dispatch" by core-bus scheduler.
struct TaskDispatch {
  api @0 :Common.ApiVersion;
  task @1 :Task;
  dispatchedAtNs @2 :UInt64;
  targetPlugin @3 :Text;  # empty = scheduler picks by capability
}
