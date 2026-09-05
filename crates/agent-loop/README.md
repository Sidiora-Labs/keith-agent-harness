# Tool admission in the agent loop

`AgentLoop::with_tool_admission` installs the runtime-owned `ToolAdmission` port.
Its `admit` method receives the existing held `SessionWriter`, the durable turn
identity and the actual tool invocation. Implementations persist frozen
admissions through that writer before returning success. The constructor stays
compatible for callers with no admission implementation; production composition
is responsible for installing the policy its tools require.

Admission runs serially immediately before each tool's dispatch, including every
thread in a parallel read batch. A `Rejected` failure becomes a durable tool
result with `NotStarted` status and effect state, and cannot emit `ToolStarted`
or reach the executor. A persistence error stops further dispatch. Reads already
started in the same batch are joined and their real results are retained.

On scheduler failure, dispatched calls without a durable result retain unknown
effects. Calls that were never dispatched are recorded as not started. These
records distinguish a failed check from a tool that might have run; they do not
replace later reconciliation or authorize automatic retries.

Focused tests exercise the actual HTTP provider adapter, session persistence,
subprocess effects and a filesystem failure. The production memory resolver and
native adapter enforcement are composed and qualified by `local-runtime`.
