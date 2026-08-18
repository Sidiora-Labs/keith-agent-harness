import {
  createEffect,
  createSignal,
  onCleanup,
  untrack,
  type Accessor
} from "solid-js"
import type {
  BootstrapData,
  BrowserProjectionBridge,
  BrowserView,
  PersonalItem,
  ResumeCursor,
  SessionSummary
} from "./types"

export type ConnectionState = "opening" | "connected" | "reconnecting" | "unavailable"

export interface KeithController {
  draft: Accessor<string>
  setDraft: (value: string) => void
  notice: Accessor<string | undefined>
  uncertain: Accessor<boolean>
  sending: Accessor<boolean>
  connectionState: Accessor<ConnectionState>
  selectSession: (sessionId: string) => void
  send: () => Promise<void>
  steer: () => Promise<void>
  stop: () => Promise<void>
  retry: () => Promise<void>
  branch: () => Promise<void>
  resume: () => Promise<void>
  newConversation: () => Promise<void>
  decide: (item: PersonalItem, allow: boolean) => Promise<void>
  exportResult: () => Promise<void>
  recover: () => void
}

interface ConnectionOptions {
  bootstrap: BootstrapData
  bridge: Accessor<BrowserProjectionBridge | undefined>
  view: Accessor<BrowserView>
  setView: (view: BrowserView) => void
  sessions: Accessor<SessionSummary[]>
  setSessions: (sessions: SessionSummary[]) => void
  selectedSession: Accessor<string | undefined>
  setSelectedSession: (sessionId: string) => void
}

const MAX_DRAFT_BYTES = 65_536
const RECONNECT_DELAYS = [400, 800, 1_600, 3_200, 6_400, 8_000] as const

export function createKeithConnection(options: ConnectionOptions): KeithController {
  const [draft, setDraftSignal] = createSignal("")
  const [notice, setNotice] = createSignal<string>()
  const [uncertain, setUncertain] = createSignal(false)
  const [sending, setSending] = createSignal(false)
  const [connectionState, setConnectionState] = createSignal<ConnectionState>("opening")
  const drafts = new Map<string, string>()
  let forceSnapshot = false

  const updateView = (next: BrowserView) => {
    options.setView(next)
    if (next.sessions?.length) {
      const current = untrack(options.sessions)
      const merged = mergeSessions(current, next.sessions)
      if (!sameSessionCatalog(current, merged)) options.setSessions(merged)
    }
  }

  const selectSession = (sessionId: string) => {
    const current = options.selectedSession()
    if (current) drafts.set(current, draft())
    options.setSelectedSession(sessionId)
    setDraftSignal(drafts.get(sessionId) ?? "")
    setNotice(undefined)
    setUncertain(false)
    if (options.view().personal?.session_id !== sessionId) {
      options.setView({ snapshot_required: true })
      forceSnapshot = true
    }
  }

  const reconnectAuthoritatively = () => {
    forceSnapshot = true
    setUncertain(true)
    setNotice(
      "The connection changed before Keith confirmed what happened. Your draft is safe while the conversation refreshes."
    )
    window.dispatchEvent(new CustomEvent("keith:refresh-connection"))
  }

  createEffect(() => {
    const bridge = options.bridge()
    const sessionId = options.selectedSession()
    const session = untrack(options.sessions).find((candidate) => candidate.session_id === sessionId)
    if (!bridge || !sessionId || !session) {
      setConnectionState(bridge ? "unavailable" : "opening")
      return
    }

    let active = true
    let socket: WebSocket | undefined
    let timer: number | undefined
    let attempt = 0

    const open = () => {
      if (!active) return
      const current = untrack(options.view)
      const exactResume =
        !forceSnapshot && current.personal?.session_id === sessionId
          ? current.resume ?? undefined
          : undefined
      setConnectionState(attempt === 0 ? "opening" : "reconnecting")
      socket = new WebSocket(eventSocketUrl(window.location.origin, session.profile_id, sessionId, exactResume))
      socket.addEventListener("open", () => {
        if (!active) return
        attempt = 0
        forceSnapshot = false
        setConnectionState("connected")
      })
      socket.addEventListener("message", (event) => {
        if (!active || typeof event.data !== "string") return
        try {
          const next = parseView(bridge.apply_wire_message(event.data))
          updateView(next)
          if (next.snapshot_required) {
            forceSnapshot = true
            socket?.close(1012, "refresh")
          } else if (next.personal?.session_id === sessionId) {
            setUncertain(false)
          }
        } catch {
          forceSnapshot = true
          socket?.close(1012, "refresh")
        }
      })
      socket.addEventListener("close", () => {
        if (!active) return
        setConnectionState("reconnecting")
        const delay = RECONNECT_DELAYS[Math.min(attempt, RECONNECT_DELAYS.length - 1)]
        attempt += 1
        timer = window.setTimeout(open, delay)
      })
    }

    const refresh = () => {
      forceSnapshot = true
      socket?.close(1012, "refresh")
    }
    window.addEventListener("keith:refresh-connection", refresh)
    open()
    onCleanup(() => {
      active = false
      if (timer !== undefined) window.clearTimeout(timer)
      window.removeEventListener("keith:refresh-connection", refresh)
      socket?.close(1000, "session changed")
    })
  })

  const sendEnvelope = async (
    build: (bridge: BrowserProjectionBridge) => string,
    clearDraft: boolean,
    successCopy: string
  ): Promise<BrowserView | undefined> => {
    const bridge = options.bridge()
    const sessionId = options.selectedSession()
    const profileId = profileFor(options.sessions(), sessionId) ?? options.bootstrap.profiles[0]?.id
    if (!bridge || !profileId || sending()) return undefined
    let envelope: string
    try {
      envelope = build(bridge)
    } catch (error) {
      setNotice(safeClientError(error))
      return undefined
    }
    setSending(true)
    setNotice(undefined)
    try {
      const response = await fetch(`/api/profiles/${encodeURIComponent(profileId)}/commands`, {
        method: "POST",
        credentials: "same-origin",
        headers: {
          "content-type": "application/json",
          "x-keith-csrf": options.bootstrap.csrf
        },
        body: envelope
      })
      if (!response.ok) {
        if (response.status >= 500) reconnectAuthoritatively()
        else setNotice("Keith could not accept that request. Nothing was changed.")
        return undefined
      }
      const next = parseView(bridge.apply_wire_message(await response.text()))
      updateView(next)
      if (next.last_command?.state === "rejected") {
        setNotice(next.last_command.message || "Keith could not accept that request.")
        return next
      }
      if (clearDraft) {
        setDraftSignal("")
        if (sessionId) drafts.set(sessionId, "")
      }
      setUncertain(false)
      setNotice(successCopy)
      return next
    } catch {
      reconnectAuthoritatively()
      return undefined
    } finally {
      setSending(false)
    }
  }

  const withSession = async (
    build: (bridge: BrowserProjectionBridge, sessionId: string) => string,
    successCopy: string,
    clearDraft = false
  ) => {
    const sessionId = options.selectedSession()
    if (!sessionId) {
      setNotice("Choose a conversation first.")
      return
    }
    await sendEnvelope((bridge) => build(bridge, sessionId), clearDraft, successCopy)
  }

  return {
    draft,
    setDraft(value) {
      const bounded = boundUtf8(value, MAX_DRAFT_BYTES)
      setDraftSignal(bounded)
      const sessionId = options.selectedSession()
      if (sessionId) drafts.set(sessionId, bounded)
    },
    notice,
    uncertain,
    sending,
    connectionState,
    selectSession,
    async send() {
      const message = draft()
      await withSession(
        (bridge, sessionId) => bridge.submit_prompt(sessionId, message),
        "Keith received your message. Progress and the result will appear here.",
        true
      )
    },
    async steer() {
      const message = draft()
      await withSession(
        (bridge, sessionId) => bridge.steer(sessionId, message),
        "Your guidance was received. The conversation will show what Keith does with it.",
        true
      )
    },
    async stop() {
      await withSession(
        (bridge, sessionId) => bridge.cancel(sessionId),
        "Keith received the stop request. The final state will appear here."
      )
    },
    async retry() {
      await withSession(
        (bridge, sessionId) => bridge.retry(sessionId),
        "The last confirmed message was sent again."
      )
    },
    async branch() {
      await withSession(
        (bridge, sessionId) => bridge.branch(sessionId),
        "A new path was requested from the last completed reply."
      )
    },
    async resume() {
      await withSession(
        (bridge, sessionId) => bridge.resume(sessionId),
        "Keith received the request to continue."
      )
    },
    async newConversation() {
      const profile = options.bootstrap.profiles[0]
      if (!profile) {
        setNotice("Keith does not have an available profile yet.")
        return
      }
      const next = await sendEnvelope(
        (bridge) => bridge.create_session(profile.id, profile.workspace_id),
        false,
        "Your new conversation is ready."
      )
      const created = next?.personal?.session_id
      if (created) selectSession(created)
    },
    async decide(item, allow) {
      if (item.reference.kind !== "confirmation" || typeof item.reference.id !== "string") {
        setNotice("That decision is no longer waiting for you.")
        return
      }
      const confirmationId = item.reference.id
      await withSession(
        (bridge, sessionId) =>
          bridge.resolve_confirmation(sessionId, confirmationId, allow),
        allow
          ? "Keith may take that step once. The outcome will appear here."
          : "Keith will not take that step."
      )
    },
    async exportResult() {
      await withSession(
        (bridge, sessionId) => bridge.export_session(sessionId),
        "Keith prepared a portable copy with its provenance."
      )
    },
    recover: reconnectAuthoritatively
  }
}

export function eventSocketUrl(
  origin: string,
  profileId: string,
  sessionId: string,
  resume?: ResumeCursor
): string {
  const url = new URL(origin)
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:"
  url.pathname = `/api/events/${encodeURIComponent(profileId)}/${encodeURIComponent(sessionId)}`
  url.search = ""
  if (resume) {
    url.searchParams.set("generation", String(resume.generation))
    url.searchParams.set("sequence", String(resume.last_sequence))
  }
  return url.toString()
}

export function mergeSessions(
  current: SessionSummary[],
  incoming: SessionSummary[]
): SessionSummary[] {
  const merged = new Map(current.map((session) => [session.session_id, session]))
  for (const session of incoming) merged.set(session.session_id, session)
  return [...merged.values()].sort((left, right) => right.updated_at.localeCompare(left.updated_at))
}

function sameSessionCatalog(left: SessionSummary[], right: SessionSummary[]): boolean {
  return left.length === right.length && left.every((session, index) => {
    const candidate = right[index]
    return candidate?.session_id === session.session_id && candidate.title === session.title &&
      candidate.state === session.state && candidate.updated_at === session.updated_at
  })
}

function profileFor(sessions: SessionSummary[], sessionId?: string): string | undefined {
  return sessions.find((session) => session.session_id === sessionId)?.profile_id
}

function parseView(encoded: string): BrowserView {
  return JSON.parse(encoded) as BrowserView
}

function safeClientError(error: unknown): string {
  return error instanceof Error && error.message
    ? error.message
    : "Keith could not prepare that request."
}

function boundUtf8(value: string, maximum: number): string {
  const encoder = new TextEncoder()
  if (encoder.encode(value).byteLength <= maximum) return value
  let output = ""
  for (const character of value) {
    if (encoder.encode(output + character).byteLength > maximum) break
    output += character
  }
  return output
}
