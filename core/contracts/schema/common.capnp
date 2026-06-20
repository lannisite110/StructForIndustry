# Common types shared across sfi-platform contracts (apiVersion 0).

@0x8f3c2a1b0e9d7461;

struct ApiVersion {
  # Matches core/contracts/VERSION (UInt16 major only for v0).
  major @0 :UInt16;
  minor @1 :UInt16;
}

enum StatusCode {
  ok @0;
  timeout @1;
  cancelled @2;
  invalidArgument @3;
  notFound @4;
  resourceExhausted @5;
  internal @6;
  unavailable @7;
}

struct TimingMetrics {
  queuedAtNs @0 :UInt64;
  startedAtNs @1 :UInt64;
  finishedAtNs @2 :UInt64;
  # finishedAtNs - startedAtNs when both are set.
  processingNs @3 :UInt64;
}

struct ResourceReq {
  cpuMillis @0 :UInt32;      # soft CPU budget per task (0 = unspecified)
  memoryBytes @1 :UInt64;    # soft memory ceiling
  gpuCount @2 :UInt8;        # 0 = CPU only
  npuCount @3 :UInt8;
}

# Open-ended key/value metadata (UTF-8 keys, opaque values).
struct StringMapEntry {
  key @0 :Text;
  value @1 :Data;
}

struct Extensions {
  entries @0 :List(StringMapEntry);
}
