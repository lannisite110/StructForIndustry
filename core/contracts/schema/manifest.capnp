# Plugin manifest — loaded by plugin-host at startup.

@0xd48b7f6a5d4c29b6;

using Common = import "common.capnp";

enum PluginKind {
  inProcess @0;   # .so via C ABI (sfi.h)
  outOfProcess @1; # executable + IPC
}

struct PluginManifest {
  name @0 :Text;
  version @1 :Text;
  api @2 :Common.ApiVersion;

  kind @3 :PluginKind;

  # Matched against Task.type (prefix or exact; host defines matching rules).
  capabilities @4 :List(Text);

  resources @5 :Common.ResourceReq;

  # inProcess: path to shared library. outOfProcess: executable path or argv[0].
  entry @6 :Text;

  # Optional argv, env hints (out-of-process).
  args @7 :List(Text);

  extensions @8 :Common.Extensions;
}

enum PluginHealthState {
  starting @0;
  healthy @1;
  degraded @2;
  unhealthy @3;
  stopped @4;
}

struct PluginHealth {
  name @0 :Text;
  state @1 :PluginHealthState;
  message @2 :Text;
  reportedAtNs @3 :UInt64;
  restartCount @4 :UInt32;
}

# Published on topic "plugin.health".
struct PluginHealthEvent {
  api @0 :Common.ApiVersion;
  health @1 :PluginHealth;
}
