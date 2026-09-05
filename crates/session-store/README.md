# Session-owned exact binding records

`SessionWriter::require_object_bindings` records a task's required entity/property
keys and the adapter argument slots that consume them. Reading
`required_object_bindings` returns the union from the complete committed session
history, including earlier branches and compacted records. Empty later requests
cannot erase a dependency. A set is bounded to 128 key/slot pairs; reading uses
the existing full-history parser, not a bounded or vector index.

The store checks the profile, workspace and session envelope. Within that
envelope, a goal identity preserves requirements across action continuations;
an action without a goal has its own requirements. The store does not authorize
cross-session goal continuation or infer entity identity.

`append_binding_admission` freezes the full required union and the selected
current references for one actual durable tool call. Several required objects
may share `read.path`; one read selects one of those objects. Each distinct
required argument slot for the invoked tool must be represented exactly once.
The owning runtime resolves the references, validates their freshness and
interprets adapter target values. Session persistence does not certify external
truth or independently revalidate memory revisions.

Admission checks the accepted action and turn, the current committed branch,
the call's unique identity, tool name and canonical JSON argument digest. It
refuses an omitted requirement, malformed reference, conflicting frozen record,
completed turn or a new admission after a tool result. Repeating an identical
admission while the turn is open returns its existing entry. A timestamp-only
retry does not append another record.

Both record types are checksummed session history entries and stay outside
conversational context. Existing entries and legacy checksums are unchanged.
Persistence failure is returned to the caller before execution can be admitted.
