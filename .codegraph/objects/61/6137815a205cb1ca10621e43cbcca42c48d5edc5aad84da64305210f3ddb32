## Product stance

Keith is personal intelligence packaged for ordinary life. The interface should make a capable system feel direct, calm, and dependable: ask for something, see what matters, step in when Keith needs a decision, and receive a finished result. Internal machinery remains available for diagnosis, but it is not the product's organizing metaphor.

This feature is deliberately limited to the standalone Keith web and terminal clients. It does not redesign the Neo/Centra unified cloud interface and it does not create a prototype or wireframe route.

## Shared state boundary

Both clients consume the existing AgentConnection contract and `ProjectionReducer`. A new shared personal-intelligence projection translates exact runtime facts into consumer groupings while retaining the stable identifiers required to issue commands. Presentation may reorder or progressively disclose facts, but it may not infer work, completion, typing, or success.

The web client uses a narrow Rust/WASM bridge for protocol decoding, reducer application, resume cursors, consumer projections, and typed command construction. Solid receives immutable serializable views. It never calls a model provider, executes a tool, or keeps a competing canonical message list. Non-streaming catalog queries may use TanStack Solid Query; transcript windowing may use TanStack Solid Virtual. The AI Solid bindings are not a replacement for Keith's protocol authority.

## Terminal experience

The terminal follows Codex's structural model. Settled conversation becomes ordinary terminal scrollback. The mutable region stays small: any uncommitted Keith tail, one truthful activity line, the composer, a notice, and an optional overlay. Sessions, commands, models, confirmations, work, memory, and diagnostics are temporary overlays rather than permanent columns.

The terminal is typography-led. User entries, Keith entries, bounded activity summaries, warnings, and terminal results use restrained role labels and spacing. Narrow terminals lose secondary hints before they lose conversation. No layout replaces the transcript with a request to resize.

## Web experience

The web information architecture has four primary destinations plus settings:

- Home is a personal brief: current focus, what needs the user, what Keith finished, what is coming up, and a direct path back into conversation.
- Conversation is the persistent relationship surface and the place new work starts.
- Work brings goals, plans, child work, tools, waits, schedules, commitments, approvals, and delivery into Active, Needs you, Upcoming, and Completed outcomes.
- Your World makes saved context, knowledge, routines, preferences, and memory changes legible and correctable.
- Settings holds models, connections, privacy, appearance, diagnostics, and advanced controls away from the everyday path.

Artifacts and outputs live beside the conversation or work that produced them. Identifiers and subsystem names are not used as primary labels.

## Visual system

`@openai/apps-sdk-ui/css` is the web CSS and token foundation. Keith adds a small adapter that aliases Apps SDK semantic tokens into product roles and then uses those roles for layout surfaces, text, focus, radius, elevation, and motion. Raw product colors do not leak into components. Background contrast, spacing, typography, and restrained elevation establish hierarchy; decorative border strokes, glow, purple gradients, and emoji status are prohibited.

The experience supports light, dark, high contrast, reduced motion, forced colors, and text zoom. Consequential controls retain text labels. Motion explains spatial change and never simulates activity.

## Qualification strategy

Qualification runs against real compiled clients. Terminal tests exercise inline viewport behavior, continuous idle event delivery, reconnect, control-sequence neutralization, width/color matrices, and restoration. Browser tests build Solid and Rust/WASM assets, serve them through the authenticated Rust server, and drive critical paths in Chromium across desktop and mobile viewports. Release and desktop packaging consume the same production assets and fail on stale generated output.
