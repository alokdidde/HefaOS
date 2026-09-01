# Semantic regression goldens

The `v0` traces are complete, finite, replayable evidence produced by the
reference subject on the deterministic mock plant. The test suite validates
every trace, replays its captured `SubjectInputV0` stream, and requires a fresh
run to compare exactly with the committed file.

These files protect the bench and the reference graph from accidental semantic
drift. They are not the independent safety oracle: the harness's per-record
safety invariants and each scenario's hand-authored expectations remain the
authority for fail-closed behavior. A deliberate graph-semantics change must
update the versioned scenario first and regenerate its golden explicitly.
