# Contributing to Keith Agent

Run `cargo ci` before publishing a change. It checks formatting, dependency direction, compilation, lints, documentation, and the real workspace test binaries. Run `cargo clean-checkout` when changing workspace structure or generated inputs.

## Test integrity

Tests exercise real Keith domain types, repositories, codecs, state machines, process boundaries, and packaged binaries. Internal collaborators are not replaced with mocks, stubs, or fakes merely to make a test pass. Deterministic providers and fault injectors are permitted only as explicit external-boundary adapters, must implement the same public trait as production adapters, and must exercise the production orchestration path. A passing test over a placeholder is not completion.

External services that cannot run hermetically use named conformance adapters and fixtures at their network or process boundary. Those adapters must preserve production serialization, validation, cancellation, timeout, and error-classification behavior. Acceptance tasks that require a real provider, process, browser, channel, or packaged binary remain incomplete until that real path passes.

