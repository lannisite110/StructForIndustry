# Result — unified result envelope returned by plugins to core-bus.

@0xc37a6e5f4c3b1a85;

using Common = import "common.capnp";
using Buffer = import "buffer.capnp";

enum ResultStatus {
  ok @0;
  error @1;
  timeout @2;
  cancelled @3;
  partial @4;  # e.g. degraded inference, some detections dropped
}

struct BoundingBox {
  x @0 :Float32;
  y @1 :Float32;
  width @2 :Float32;
  height @3 :Float32;
}

struct Detection {
  classId @0 :UInt32;
  label @1 :Text;
  score @2 :Float32;
  bbox @3 :BoundingBox;

  # Optional mask or thumbnail as shared buffer.
  mask @4 :Buffer.SharedBuffer;

  extensions @5 :Common.Extensions;
}

struct DetectionList {
  frameId @0 :UInt64;
  sourceId @1 :Text;
  detections @2 :List(Detection);
}

struct MetricValue {
  name @0 :Text;
  value @1 :Float64;
  unit @2 :Text;
}

struct MetricsPayload {
  frameId @0 :UInt64;
  values @1 :List(MetricValue);
}

struct TensorOutput {
  name @0 :Text;
  dtype @1 :Text;   # "f32", "i64", ...
  shape @2 :List(UInt32);
  # Large tensor data via shared buffer; small tensors may use inline data.
  buffer @3 :Buffer.SharedBuffer;
  inlineData @4 :Data;
}

struct InferOutput {
  modelId @0 :Text;
  modelVersion @1 :Text;
  outputs @2 :List(TensorOutput);
}

struct ResultPayload {
  union {
    none @0 :Void;
    detections @1 :DetectionList;
    metrics @2 :MetricsPayload;
    infer @3 :InferOutput;
    raw @4 :Data;
  }
}

struct Result {
  taskId @0 :UInt64;
  status @1 :ResultStatus;

  # Machine-readable error when status != ok.
  code @2 :Common.StatusCode;
  message @3 :Text;

  payload @4 :ResultPayload;
  timing @5 :Common.TimingMetrics;

  # Plugin that produced this result.
  pluginName @6 :Text;
  pluginVersion @7 :Text;

  extensions @8 :Common.Extensions;
}

# Published on topic "task.done".
struct ResultEvent {
  api @0 :Common.ApiVersion;
  result @1 :Result;
  publishedAtNs @2 :UInt64;
}
