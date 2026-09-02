# Gate 0 Copper v1.1.1 raw evidence record

**Artifact:** Gate 0 / 0.1 direct hand-written Copper spike
**Review disposition:** Accepted as experimental evidence only
**HefaOS commit:** `8b79968e416be68c2faabcd908ae0c9fe7528512`
**Copper source:** `v1.1.1` / `fc2ebc4fe3583d1f433b75898ad7c9e4dd9e6af2`
**Rust:** `1.95.0`

The retained raw bundle is deliberately kept outside Git because it contains the
fetched upstream checkout and is 1.8 GiB. It is available only at the
repository-local evidence path below; no clone-portable archive location has
yet been published. The recorded tree digest identifies its reviewed contents:

`evidence/gate-0-copper/20260901T000000Z-8b79968`

The SHA-256 of the sorted `sha256sum` listing of every regular file in that
bundle is:

```text
6720f26bb34a0f473bd51111af53f324b5562cf07b2ac99d3983228e6c91f13f
```

The bundle retains command, stdout, stderr, and status files for all 24 required
commands. Every status is `0`, including the direct upstream `cu-run-in-sim`,
the complete frozen corpus and replay, nominal timing, formatting, workspace
clippy, and workspace tests. It contains twelve retained semantic traces and
their digests, plus the generated Copper configuration and log digests.

The bridge loopback observed ping/pong sequence values `0` through `6`; its
processes end only through their expected timeouts. The bridge remains rejected
from SO-101 control admission because it uses a bincode/`Vec` copy and has no
declared bounded queue, schema, epoch, or pool policy.

This record does not qualify hardware timing, production replay, zero-copy IPC,
or safety, and its unarchived bundle must not be represented as accepted
clone-portable raw provenance. Its virtual scope and frozen comparison workload
are now recorded by [Gate 0 artifact 0.2](../gate-0-scope-fixture-lock.md).
Gate 0 remains incomplete pending the first-target plan, the ROS comparison
protocol, the safety-controller decision, and a durable archive; a ROS bridge
is explicitly deferred to Gate 6.
