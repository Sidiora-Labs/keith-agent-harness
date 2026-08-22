export type Timestamp = number | string

export interface ProtocolVersion {
  major: number
  minor: number
}

export interface ProfileSummary {
  id: string
  workspace_id: string
  display_name: string
  enabled: boolean
}

export interface SessionSummary {
  session_id: string
  root_tree_id: string
  profile_id: string
  title?: string | null
  state: string
  updated_at: Timestamp
}

export interface BootstrapData {
  protocol: ProtocolVersion
  csrf: string
  profiles: ProfileSummary[]
  sessions: SessionSummary[]
}

export interface MessageProjection {
  message_id: string
  final_id?: string | null
  role: 'user' | 'assistant' | 'tool' | 'system'
  text: string
  committed: boolean
}

export interface ActionProjection {
  action_id: string
  source: string
  state: string
  created_at: Timestamp
}

export interface ToolProjection {
  tool_call_id: string
  tool?: string
  state: string
  terminal: boolean
  [key: string]: unknown
}

export interface ConfirmationProjection {
  confirmation_id: string
  summary: string
}

export interface TerminalProjection {
  session_id: string
  turn_id: string
  final_id: string
  status: 'completed' | 'failed' | 'cancelled' | 'exhausted'
  execution_succeeded: boolean
  final_created: boolean
  artifacts_persisted: boolean
  delivery_enqueued: boolean
  delivery_acknowledged: boolean
  detail?: string | null
}

export interface SessionSnapshot {
  session: SessionSummary
  generation: number
  through_sequence: number
  active_action?: ActionProjection | null
  actions: ActionProjection[]
  messages: MessageProjection[]
  goals: Array<Record<string, unknown>>
  plans: Array<Record<string, unknown>>
  children: Array<Record<string, unknown>>
  kernels: Array<Record<string, unknown>>
  commitments: Array<Record<string, unknown>>
  schedules: Array<Record<string, unknown>>
  tools: ToolProjection[]
  confirmations: ConfirmationProjection[]
  waits: Array<Record<string, unknown>>
  deliveries: Array<Record<string, unknown>>
  memory_changes: Array<Record<string, unknown>>
  usage: Record<string, number>
  presence: {
    session_id: string
    state: string
    updated_at: Timestamp
    safe_error?: string | null
    [key: string]: unknown
  }
  terminal?: TerminalProjection | null
  revision: number
}

export interface MemoryResult {
  source: string
  excerpt: string
  score_micros: number
}

export type Command = { command: string; parameters?: unknown }

export interface CommandResult {
  protocol: ProtocolVersion
  command_id: string
  completed_at: Timestamp
  result:
    | { status: 'accepted'; payload: { action_id: string | null } }
    | { status: 'data'; payload: { kind: string; value: unknown } }
    | {
        status: 'rejected'
        payload: {
          error?: { code?: string; message?: string; safe_message?: string; retryable?: boolean }
        }
      }
}

export interface CommandEnvelope {
  protocol: ProtocolVersion
  command_id: string
  client_id: string
  sent_at: number
  session_id: string | null
  command: Command
}

export interface LiveRunProjection {
  command_id: string | null
  phase: 'sending' | 'accepted' | 'thinking' | 'responding' | 'using_tools' | 'finalizing'
  turn: number | null
  detail: string | null
  tools: ToolProjection[]
}

export interface ProjectionState {
  snapshot: SessionSnapshot | null
  sessions: SessionSummary[]
  generation: number | null
  sequence: number | null
  snapshotRequired: boolean
  terminal: TerminalProjection | null
  lastCommand: CommandResult | null
  liveRun: LiveRunProjection | null
}

export class KeithApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
    readonly uncertain = false,
  ) {
    super(message)
  }
}

const CROCKFORD = '0123456789ABCDEFGHJKMNPQRSTVWXYZ'
const CLIENT_ID = createUlid()

export function createUlid(now = Date.now(), random = crypto.getRandomValues(new Uint8Array(16))): string {
  let time = BigInt(now)
  const output = Array<string>(26)
  for (let index = 9; index >= 0; index -= 1) {
    output[index] = CROCKFORD[Number(time & 31n)]!
    time >>= 5n
  }
  for (let index = 0; index < 16; index += 1) {
    output[index + 10] = CROCKFORD[random[index]! & 31]!
  }
  return output.join('')
}

export function commandEnvelope(
  protocol: ProtocolVersion,
  sessionId: string | null,
  command: Command,
): CommandEnvelope {
  return {
    protocol,
    command_id: createUlid(),
    client_id: CLIENT_ID,
    sent_at: Date.now(),
    session_id: sessionId,
    command,
  }
}

export async function getBootstrap(session?: string): Promise<BootstrapData> {
  const query = session ? `?session=${encodeURIComponent(session)}` : ''
  const response = await fetch(`/api/bootstrap${query}`, {
    credentials: 'same-origin',
    headers: { accept: 'application/json' },
    cache: 'no-store',
  })
  if (!response.ok) throw new KeithApiError(response.status, await safeError(response))
  return (await response.json()) as BootstrapData
}

export async function executeCommand(
  bootstrap: BootstrapData,
  profileId: string,
  envelope: CommandEnvelope,
  onWireMessage?: (encoded: string) => void,
): Promise<CommandResult> {
  const encoded = JSON.stringify(envelope)
  let response: Response | undefined
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      response = await fetch(`/api/profiles/${encodeURIComponent(profileId)}/commands`, {
        method: 'POST',
        credentials: 'same-origin',
        headers: {
          accept: onWireMessage ? 'text/event-stream' : 'application/json',
          'content-type': 'application/json',
          'x-keith-csrf': bootstrap.csrf,
        },
        body: encoded,
      })
    } catch {
      response = undefined
    }
    if (response?.ok || (response && response.status < 500)) break
    await wait(300 * 2 ** attempt)
  }
  if (!response) {
    throw new KeithApiError(
      0,
      'The connection changed before Keith confirmed the request. Refresh the conversation before retrying.',
      true,
    )
  }
  if (!response.ok) {
    throw new KeithApiError(response.status, await safeError(response), response.status >= 500)
  }
  if (onWireMessage) {
    return readCommandStream(response, onWireMessage)
  }
  const wire = (await response.json()) as { message?: string; payload?: CommandResult }
  if (wire.message !== 'command_result' || !wire.payload) {
    throw new KeithApiError(502, 'Keith returned an invalid command response.')
  }
  if (wire.payload.result.status === 'rejected') {
    const error = wire.payload.result.payload.error
    throw new KeithApiError(
      409,
      error?.safe_message || error?.message || 'Keith rejected the request.',
    )
  }
  return wire.payload
}

export function dataFromResult<T>(result: CommandResult, kind: string): T | undefined {
  return result.result.status === 'data' && result.result.payload.kind === kind
    ? (result.result.payload.value as T)
    : undefined
}

export function eventSocketUrl(
  origin: string,
  profileId: string,
  sessionId: string,
  cursor?: { generation: number; sequence: number },
): string {
  const url = new URL(origin)
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
  url.pathname = `/api/events/${encodeURIComponent(profileId)}/${encodeURIComponent(sessionId)}`
  url.search = ''
  if (cursor) {
    url.searchParams.set('generation', String(cursor.generation))
    url.searchParams.set('sequence', String(cursor.sequence))
  }
  return url.toString()
}

export function emptyProjection(sessions: SessionSummary[] = []): ProjectionState {
  return {
    snapshot: null,
    sessions,
    generation: null,
    sequence: null,
    snapshotRequired: false,
    terminal: null,
    lastCommand: null,
    liveRun: null,
  }
}

export function beginLiveRun(current: ProjectionState, commandId: string): ProjectionState {
  return {
    ...current,
    liveRun: {
      command_id: commandId,
      phase: 'sending',
      turn: null,
      detail: null,
      tools: [],
    },
  }
}

export function applyWireMessage(current: ProjectionState, encoded: string): ProjectionState {
  const wire = JSON.parse(encoded) as { message?: string; payload?: unknown }
  if (wire.message === 'server_hello') return current
  if (wire.message === 'command_result') {
    const result = wire.payload as CommandResult
    const snapshot = dataFromResult<SessionSnapshot>(result, 'snapshot')
    const sessions = dataFromResult<SessionSummary[]>(result, 'sessions')
    if (snapshot) {
      return { ...installSnapshot({ ...current, lastCommand: result }, snapshot), liveRun: null }
    }
    return {
      ...current,
      lastCommand: result,
      liveRun: null,
      sessions: sessions ? mergeSessions(current.sessions, sessions) : current.sessions,
    }
  }
  if (wire.message === 'snapshot') {
    const payload = object(wire.payload)
    const snapshot = ('snapshot' in payload ? payload.snapshot : payload) as SessionSnapshot
    return installSnapshot(current, snapshot)
  }
  if (wire.message === 'terminal') {
    const payload = object(wire.payload)
    const terminal = ('terminal' in payload ? payload.terminal : payload) as TerminalProjection
    const generation = numeric(payload.generation) ?? current.generation
    const sequence = numeric(payload.sequence) ?? current.sequence
    return {
      ...current,
      generation,
      sequence,
      terminal,
      snapshot: current.snapshot ? { ...current.snapshot, terminal } : null,
    }
  }
  if (wire.message !== 'event') return current

  const frame = object(wire.payload)
  const generation = numeric(frame.generation)
  const sequence = numeric(frame.sequence)
  const firstSequence = numeric(frame.first_sequence) ?? sequence
  if (generation === null || sequence === null) return { ...current, snapshotRequired: true }
  if (current.generation !== null && generation < current.generation) return current
  if (
    current.generation === generation &&
    current.sequence !== null &&
    firstSequence !== null &&
    firstSequence > current.sequence + 1
  ) {
    return { ...current, snapshotRequired: true }
  }
  const event = object(frame.event)
  const kind = typeof event.event === 'string' ? event.event : ''
  const payload = object(event.payload)
  if (
    current.generation === generation &&
    current.sequence !== null &&
    sequence <= current.sequence
  ) {
    return current
  }
  if (kind === 'snapshot') return installSnapshot(current, event.payload as SessionSnapshot)
  if (!current.snapshot) {
    return { ...current, generation, sequence, snapshotRequired: true }
  }

  let snapshot = current.snapshot
  let liveRun = current.liveRun
  if (kind === 'command_accepted') {
    liveRun = {
      command_id: String(payload.command_id ?? liveRun?.command_id ?? ''),
      phase: 'accepted',
      turn: liveRun?.turn ?? null,
      detail: null,
      tools: liveRun?.tools ?? [],
    }
  } else if (kind === 'agent_activity') {
    liveRun = projectAgentActivity(liveRun, payload)
  } else if (kind === 'session_changed') {
    snapshot = { ...snapshot, session: payload as unknown as SessionSummary }
  } else if (kind === 'action_queued' || kind === 'action_started' || kind === 'action_finished') {
    const action = payload as unknown as ActionProjection
    snapshot = {
      ...snapshot,
      active_action: kind === 'action_started' ? action : kind === 'action_finished' ? null : snapshot.active_action,
      actions: upsert(snapshot.actions, action, 'action_id'),
    }
  } else if (kind === 'assistant_delta') {
    const messageId = String(payload.message_id ?? '')
    const delta = String(payload.text ?? '')
    if (messageId && delta) {
      const existing = snapshot.messages.find((message) => message.message_id === messageId)
      const next: MessageProjection = existing
        ? { ...existing, text: `${existing.text}${delta}` }
        : { message_id: messageId, role: 'assistant', text: delta, committed: false }
      snapshot = { ...snapshot, messages: upsert(snapshot.messages, next, 'message_id') }
    }
  } else if (kind === 'message_committed') {
    snapshot = {
      ...snapshot,
      messages: upsert(snapshot.messages, payload as unknown as MessageProjection, 'message_id'),
    }
  } else if (kind === 'tool_changed') {
    const tool = payload as unknown as ToolProjection
    liveRun = {
      command_id: liveRun?.command_id ?? null,
      phase: tool.terminal ? 'thinking' : 'using_tools',
      turn: liveRun?.turn ?? null,
      detail: tool.tool ?? null,
      tools: upsert(liveRun?.tools ?? [], tool, 'tool_call_id'),
    }
    snapshot = {
      ...snapshot,
      tools: upsert(snapshot.tools, tool, 'tool_call_id'),
    }
  } else if (kind === 'goal_changed') {
    snapshot = { ...snapshot, goals: upsertUnknown(snapshot.goals, payload, 'goal_id') }
  } else if (kind === 'plan_changed') {
    snapshot = { ...snapshot, plans: upsertUnknown(snapshot.plans, payload, 'plan_id') }
  } else if (kind === 'child_changed') {
    snapshot = { ...snapshot, children: upsertUnknown(snapshot.children, payload, 'child_id') }
  } else if (kind === 'kernel_changed') {
    snapshot = { ...snapshot, kernels: upsertUnknown(snapshot.kernels, payload, 'kernel_id') }
  } else if (kind === 'commitment_changed') {
    snapshot = {
      ...snapshot,
      commitments: upsertUnknown(snapshot.commitments, payload, 'commitment_id'),
    }
  } else if (kind === 'schedule_changed') {
    snapshot = { ...snapshot, schedules: upsertUnknown(snapshot.schedules, payload, 'job_id') }
  } else if (kind === 'wait_changed') {
    snapshot = { ...snapshot, waits: upsertUnknown(snapshot.waits, payload, 'wait_id') }
  } else if (kind === 'delivery_changed') {
    snapshot = { ...snapshot, deliveries: upsertUnknown(snapshot.deliveries, payload, 'delivery_id') }
  } else if (kind === 'memory_changed') {
    snapshot = {
      ...snapshot,
      memory_changes: [...snapshot.memory_changes.slice(-199), payload],
    }
  } else if (kind === 'usage_changed') {
    snapshot = { ...snapshot, usage: payload as Record<string, number> }
  } else if (kind === 'presence_changed') {
    snapshot = { ...snapshot, presence: payload as SessionSnapshot['presence'] }
  } else if (kind === 'confirmation_requested') {
    const confirmation = payload as unknown as ConfirmationProjection
    snapshot = {
      ...snapshot,
      confirmations: upsert(snapshot.confirmations, confirmation, 'confirmation_id'),
    }
  } else if (kind === 'confirmation_resolved') {
    snapshot = {
      ...snapshot,
      confirmations: snapshot.confirmations.filter(
        (item) => item.confirmation_id !== payload.confirmation_id,
      ),
    }
  } else if (kind === 'turn_terminal') {
    snapshot = { ...snapshot, terminal: payload as unknown as TerminalProjection }
  } else if ((kind === 'warning' || kind === 'error') && typeof payload.safe_message === 'string') {
    snapshot = {
      ...snapshot,
      presence: { ...snapshot.presence, safe_error: payload.safe_message },
    }
  }

  return {
    ...current,
    snapshot: { ...snapshot, generation, through_sequence: sequence },
    sessions: mergeSessions(current.sessions, [snapshot.session]),
    generation,
    sequence,
    snapshotRequired: false,
    terminal: snapshot.terminal ?? current.terminal,
    liveRun,
  }
}

function projectAgentActivity(
  current: LiveRunProjection | null,
  payload: Record<string, unknown>,
): LiveRunProjection | null {
  const kind = object(payload.kind)
  const activity = typeof kind.activity === 'string' ? kind.activity : ''
  const detail = object(kind.payload)
  const base: LiveRunProjection = current ?? {
    command_id: null,
    phase: 'thinking',
    turn: null,
    detail: null,
    tools: [],
  }
  if (activity === 'agent_started') return { ...base, phase: 'thinking', detail: null }
  if (activity === 'turn_started') {
    return {
      ...base,
      phase: 'thinking',
      turn: numeric(detail.number),
      detail: null,
    }
  }
  if (activity === 'assistant_started') return { ...base, phase: 'responding', detail: null }
  if (activity === 'assistant_completed') return { ...base, phase: 'finalizing' }
  if (activity === 'strategy_changed') {
    return {
      ...base,
      phase: 'thinking',
      detail: typeof detail.reason === 'string' ? detail.reason : 'Adjusting approach',
    }
  }
  if (activity === 'turn_ended') return { ...base, phase: 'finalizing', detail: null }
  if (activity === 'agent_ended') return null
  return base
}

export function mergeSessions(
  current: SessionSummary[],
  incoming: SessionSummary[],
): SessionSummary[] {
  const merged = new Map(current.map((session) => [session.session_id, session]))
  for (const session of incoming) merged.set(session.session_id, session)
  return [...merged.values()].sort(
    (left, right) => timestamp(right.updated_at) - timestamp(left.updated_at),
  )
}

export function visibleUserText(text: string): string {
  const opening = '<openai_compatible_conversation>'
  const closing = '</openai_compatible_conversation>'
  const start = text.indexOf(opening)
  const end = text.indexOf(closing, start + opening.length)
  if (start < 0 || end < 0) return text
  try {
    const transcript = JSON.parse(text.slice(start + opening.length, end)) as unknown
    if (!Array.isArray(transcript)) return text
    for (let index = transcript.length - 1; index >= 0; index -= 1) {
      const message = object(transcript[index])
      if (message.role === 'user' && typeof message.content === 'string') return message.content
    }
  } catch {}
  return text
}

function installSnapshot(current: ProjectionState, snapshot: SessionSnapshot): ProjectionState {
  return {
    ...current,
    snapshot,
    sessions: mergeSessions(current.sessions, [snapshot.session]),
    generation: snapshot.generation,
    sequence: snapshot.through_sequence,
    snapshotRequired: false,
    terminal: snapshot.terminal ?? null,
  }
}

function upsert<T, K extends keyof T>(
  items: T[],
  item: T,
  key: K,
): T[] {
  const index = items.findIndex((candidate) => candidate[key] === item[key])
  if (index < 0) return [...items, item]
  const next = [...items]
  next[index] = item
  return next
}

function upsertUnknown(
  items: Array<Record<string, unknown>>,
  item: Record<string, unknown>,
  key: string,
): Array<Record<string, unknown>> {
  const index = items.findIndex((candidate) => candidate[key] === item[key])
  if (index < 0) return [...items, item]
  const next = [...items]
  next[index] = item
  return next
}

function object(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {}
}

function numeric(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

function timestamp(value: Timestamp): number {
  if (typeof value === 'number') return value
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : 0
}

export function takeSseData(buffer: string): { data: string[]; rest: string } {
  const data: string[] = []
  let rest = buffer
  while (true) {
    const separator = /\r?\n\r?\n/.exec(rest)
    if (!separator || separator.index === undefined) break
    const frame = rest.slice(0, separator.index)
    rest = rest.slice(separator.index + separator[0].length)
    const payload = frame
      .split(/\r?\n/)
      .filter((line) => line.startsWith('data:'))
      .map((line) => line.slice(5).replace(/^ /, ''))
      .join('\n')
    if (payload) data.push(payload)
  }
  return { data, rest }
}

async function readCommandStream(
  response: Response,
  onWireMessage: (encoded: string) => void,
): Promise<CommandResult> {
  if (!response.body) {
    throw new KeithApiError(502, 'Keith opened an empty event stream.', true)
  }
  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''
  const terminalResult: { value: CommandResult | null } = { value: null }
  const accept = (encoded: string) => {
    let wire: { message?: string; payload?: unknown }
    try {
      wire = JSON.parse(encoded) as { message?: string; payload?: unknown }
    } catch {
      throw new KeithApiError(502, 'Keith returned a malformed stream event.', true)
    }
    if (wire.message === 'stream_error') {
      const payload = object(wire.payload)
      throw new KeithApiError(
        503,
        typeof payload.safe_message === 'string'
          ? payload.safe_message
          : "Keith's event stream became unavailable.",
        true,
      )
    }
    if (wire.message === 'command_result') {
      terminalResult.value = wire.payload as CommandResult
      return
    }
    onWireMessage(encoded)
  }
  try {
    while (true) {
      const { done, value } = await reader.read()
      buffer += decoder.decode(value, { stream: !done })
      const parsed = takeSseData(buffer)
      buffer = parsed.rest
      for (const encoded of parsed.data) accept(encoded)
      if (done) break
    }
  } catch (error) {
    if (error instanceof KeithApiError) throw error
    throw new KeithApiError(
      0,
      'The live stream changed before Keith confirmed completion. Reconnect to recover the authoritative turn.',
      true,
    )
  } finally {
    reader.releaseLock()
  }
  const result = terminalResult.value
  if (!result) {
    throw new KeithApiError(
      502,
      'Keith closed the live stream before returning a terminal command result.',
      true,
    )
  }
  if (result.result.status === 'rejected') {
    const error = result.result.payload.error
    throw new KeithApiError(
      409,
      error?.safe_message || error?.message || 'Keith rejected the request.',
    )
  }
  return result
}

async function safeError(response: Response): Promise<string> {
  try {
    const body = await response.text()
    if (!body) return `Keith request failed (${response.status}).`
    try {
      const parsed = JSON.parse(body) as { error?: { message?: string } }
      return parsed.error?.message || body
    } catch {
      return body
    }
  } catch {
    return `Keith request failed (${response.status}).`
  }
}

function wait(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds))
}
