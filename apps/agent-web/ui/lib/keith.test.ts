import { describe, expect, it } from 'vitest'
import {
  applyWireMessage,
  beginLiveRun,
  commandEnvelope,
  createUlid,
  emptyProjection,
  eventSocketUrl,
  mergeSessions,
  takeSseData,
  visibleUserText,
  type SessionSnapshot,
} from './keith'

const SESSION_ID = '01J00000000000000000000001'
const PROFILE_ID = '01J00000000000000000000002'

function snapshot(): SessionSnapshot {
  return {
    session: {
      session_id: SESSION_ID,
      root_tree_id: '01J00000000000000000000003',
      profile_id: PROFILE_ID,
      title: 'Protocol test',
      state: 'ready',
      updated_at: 1_000,
    },
    generation: 7,
    through_sequence: 10,
    active_action: null,
    actions: [],
    messages: [],
    goals: [],
    plans: [],
    children: [],
    kernels: [],
    commitments: [],
    schedules: [],
    tools: [],
    confirmations: [],
    waits: [],
    deliveries: [],
    memory_changes: [],
    usage: {},
    presence: { session_id: SESSION_ID, state: 'ready', updated_at: 1_000 },
    terminal: null,
    revision: 1,
  }
}

describe('Keith browser protocol', () => {
  it('creates sortable Rust-compatible command identifiers and envelopes', () => {
    const id = createUlid(1_700_000_000_000, new Uint8Array(16).fill(9))
    expect(id).toMatch(/^[0-9A-HJKMNP-TV-Z]{26}$/)
    const envelope = commandEnvelope(
      { major: 1, minor: 0 },
      SESSION_ID,
      { command: 'resume_session', parameters: { session_id: SESSION_ID } },
    )
    expect(envelope).toMatchObject({
      protocol: { major: 1, minor: 0 },
      session_id: SESSION_ID,
      command: { command: 'resume_session', parameters: { session_id: SESSION_ID } },
    })
    expect(envelope.command_id).toHaveLength(26)
    expect(envelope.client_id).toHaveLength(26)
  })

  it('builds an authenticated resumable websocket URL without leaking prior query data', () => {
    expect(
      eventSocketUrl('https://keith.local/?secret=discard', PROFILE_ID, SESSION_ID, {
        generation: 7,
        sequence: 10,
      }),
    ).toBe(
      `wss://keith.local/api/events/${PROFILE_ID}/${SESSION_ID}?generation=7&sequence=10`,
    )
  })

  it('installs a snapshot then projects authoritative deltas and commits', () => {
    let projection = applyWireMessage(
      emptyProjection(),
      JSON.stringify({ message: 'snapshot', payload: { snapshot: snapshot() } }),
    )
    projection = applyWireMessage(
      projection,
      JSON.stringify({
        message: 'event',
        payload: {
          generation: 7,
          first_sequence: 11,
          sequence: 11,
          event: {
            event: 'assistant_delta',
            payload: { message_id: 'assistant-1', text: 'Hello' },
          },
        },
      }),
    )
    projection = applyWireMessage(
      projection,
      JSON.stringify({
        message: 'event',
        payload: {
          generation: 7,
          first_sequence: 12,
          sequence: 12,
          event: {
            event: 'message_committed',
            payload: {
              message_id: 'assistant-1',
              final_id: 'final-1',
              role: 'assistant',
              text: 'Hello, world.',
              committed: true,
            },
          },
        },
      }),
    )
    expect(projection.snapshotRequired).toBe(false)
    expect(projection.sequence).toBe(12)
    expect(projection.snapshot?.messages).toEqual([
      {
        message_id: 'assistant-1',
        final_id: 'final-1',
        role: 'assistant',
        text: 'Hello, world.',
        committed: true,
      },
    ])
  })

  it('projects prompt acceptance, agent phases, and tool activity in real time', () => {
    let projection = beginLiveRun(
      applyWireMessage(
        emptyProjection(),
        JSON.stringify({ message: 'snapshot', payload: { snapshot: snapshot() } }),
      ),
      'command-1',
    )
    expect(projection.liveRun?.phase).toBe('sending')
    projection = applyWireMessage(
      projection,
      JSON.stringify({
        message: 'event',
        payload: {
          generation: 7,
          first_sequence: 11,
          sequence: 11,
          event: {
            event: 'command_accepted',
            payload: { command_id: 'command-1' },
          },
        },
      }),
    )
    projection = applyWireMessage(
      projection,
      JSON.stringify({
        message: 'event',
        payload: {
          generation: 7,
          first_sequence: 12,
          sequence: 12,
          event: {
            event: 'agent_activity',
            payload: {
              session_id: SESSION_ID,
              turn_id: 'turn-1',
              sequence: 2,
              kind: { activity: 'turn_started', payload: { number: 1 } },
            },
          },
        },
      }),
    )
    projection = applyWireMessage(
      projection,
      JSON.stringify({
        message: 'event',
        payload: {
          generation: 7,
          first_sequence: 13,
          sequence: 13,
          event: {
            event: 'tool_changed',
            payload: {
              tool_call_id: 'tool-1',
              tool: 'workspace_read',
              state: 'running',
              terminal: false,
            },
          },
        },
      }),
    )
    expect(projection.liveRun).toMatchObject({
      command_id: 'command-1',
      phase: 'using_tools',
      turn: 1,
      tools: [{ tool_call_id: 'tool-1', state: 'running' }],
    })
  })

  it('ignores replayed events after the executing stream already applied them', () => {
    let projection = applyWireMessage(
      emptyProjection(),
      JSON.stringify({ message: 'snapshot', payload: { snapshot: snapshot() } }),
    )
    const delta = JSON.stringify({
      message: 'event',
      payload: {
        generation: 7,
        first_sequence: 11,
        sequence: 11,
        event: {
          event: 'assistant_delta',
          payload: { message_id: 'assistant-1', text: 'Hello' },
        },
      },
    })
    projection = applyWireMessage(projection, delta)
    projection = applyWireMessage(projection, delta)
    expect(projection.snapshot?.messages[0]?.text).toBe('Hello')
  })

  it('decodes fragmented SSE frames and preserves the incomplete tail', () => {
    const parsed = takeSseData(
      ': keep-alive\n\ndata: {"message":"event"}\n\ndata: first\ndata: second\n\ndata: partial',
    )
    expect(parsed.data).toEqual(['{"message":"event"}', 'first\nsecond'])
    expect(parsed.rest).toBe('data: partial')
  })

  it('requests a fresh snapshot when the event stream has a gap', () => {
    const installed = applyWireMessage(
      emptyProjection(),
      JSON.stringify({ message: 'snapshot', payload: { snapshot: snapshot() } }),
    )
    const gapped = applyWireMessage(
      installed,
      JSON.stringify({
        message: 'event',
        payload: {
          generation: 7,
          first_sequence: 13,
          sequence: 13,
          event: { event: 'presence_changed', payload: {} },
        },
      }),
    )
    expect(gapped.snapshotRequired).toBe(true)
    expect(gapped.sequence).toBe(10)
  })

  it('keeps sessions ordered and unwraps compatible user ingress', () => {
    const sessions = mergeSessions(
      [snapshot().session],
      [{ ...snapshot().session, session_id: 'newer', title: 'Newer', updated_at: 2_000 }],
    )
    expect(sessions.map((session) => session.title)).toEqual(['Newer', 'Protocol test'])
    const wrapped = [
      '<openai_compatible_conversation>',
      JSON.stringify([
        { role: 'assistant', content: 'Earlier' },
        { role: 'user', content: 'Visible request' },
      ]),
      '</openai_compatible_conversation>',
    ].join('')
    expect(visibleUserText(wrapped)).toBe('Visible request')
  })
})
