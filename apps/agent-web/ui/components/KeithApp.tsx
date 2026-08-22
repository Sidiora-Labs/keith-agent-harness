'use client'

import {
  FormEvent,
  PointerEvent as ReactPointerEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { Streamdown } from 'streamdown'
import {
  Activity,
  Agent,
  Archive,
  ArrowUp,
  Calendar,
  Chat,
  Check,
  CheckCircle,
  Code,
  Copy,
  Download,
  File,
  Goal,
  Memory,
  Menu,
  Monitor,
  More,
  Plus,
  Refresh,
  Search,
  Settings,
  Stop,
  Tools,
  Warning,
  X,
} from '@/components/icons'
import {
  applyWireMessage,
  beginLiveRun,
  commandEnvelope,
  dataFromResult,
  emptyProjection,
  eventSocketUrl,
  executeCommand,
  getBootstrap,
  mergeSessions,
  visibleUserText,
  type BootstrapData,
  type Command,
  type CommandResult,
  type MemoryResult,
  type MessageProjection,
  type LiveRunProjection,
  type ProjectionState,
  type SessionSnapshot,
  type SessionSummary,
} from '@/lib/keith'

type SheetName = 'sessions' | 'work' | 'memory' | 'schedule' | 'settings' | null
type ConnectionState = 'opening' | 'connected' | 'reconnecting' | 'unavailable'

const RECONNECT_DELAYS = [400, 800, 1_600, 3_200, 6_400, 8_000]
const SPLIT_RATIO_KEY = 'keith:workspace-split-ratio:v1'
const QUICK_ACTIONS = [
  {
    title: 'Plan a project',
    description: 'Turn an outcome into milestones, risks, and a concrete first move.',
    prompt: 'Help me plan a project. Start by asking for the outcome and constraints.',
    icon: Goal,
  },
  {
    title: 'Research a question',
    description: 'Investigate a topic, compare evidence, and return cited findings.',
    prompt: 'Research this with current sources and give me a concise evidence-backed answer: ',
    icon: Search,
  },
  {
    title: 'Work with files',
    description: 'Create, inspect, transform, or organize files in Keith’s workspace.',
    prompt: 'Use your file tools to help me with this workspace task: ',
    icon: File,
  },
  {
    title: 'Build or fix code',
    description: 'Inspect the real project, implement the change, and verify it.',
    prompt: 'Inspect the current project and implement this carefully: ',
    icon: Code,
  },
]

export function KeithApp() {
  const [auth, setAuth] = useState<'loading' | 'login' | 'ready' | 'error'>('loading')
  const [bootstrap, setBootstrap] = useState<BootstrapData | null>(null)
  const [projection, setProjection] = useState<ProjectionState>(() => emptyProjection())
  const [selectedSession, setSelectedSession] = useState<string | null>(null)
  const [connection, setConnection] = useState<ConnectionState>('opening')
  const [sheet, setSheet] = useState<SheetName>(null)
  const [sidebarOpen, setSidebarOpen] = useState(false)
  const [workspaceOpen, setWorkspaceOpen] = useState(false)
  const [fullscreenWork, setFullscreenWork] = useState(false)
  const [splitRatio, setSplitRatio] = useState(42)
  const [draft, setDraft] = useState('')
  const [sending, setSending] = useState(false)
  const [creating, setCreating] = useState(false)
  const [notice, setNotice] = useState<string | null>(null)
  const [memoryResults, setMemoryResults] = useState<MemoryResult[]>([])
  const [lastPrompt, setLastPrompt] = useState<string | null>(null)
  const [pendingPrompt, setPendingPrompt] = useState<string | null>(null)
  const projectionRef = useRef(projection)
  const draftBySession = useRef(new Map<string, string>())
  const rootRef = useRef<HTMLElement>(null)
  const dragging = useRef(false)

  useEffect(() => {
    projectionRef.current = projection
  }, [projection])

  const loadBootstrap = useCallback(async () => {
    try {
      const requested = new URLSearchParams(window.location.search).get('session') ?? undefined
      const data = await getBootstrap(requested)
      const sessions = mergeSessions([], data.sessions)
      const selected = sessions[0]?.session_id ?? null
      setBootstrap(data)
      setProjection(emptyProjection(sessions))
      setSelectedSession(selected)
      setAuth('ready')
      setNotice(null)
    } catch (error) {
      if (error instanceof Error && 'status' in error && error.status === 401) setAuth('login')
      else setAuth('error')
    }
  }, [])

  useEffect(() => {
    void loadBootstrap()
  }, [loadBootstrap])

  useEffect(() => {
    try {
      const stored = Number(window.localStorage.getItem(SPLIT_RATIO_KEY))
      if (Number.isFinite(stored)) setSplitRatio(clamp(stored, 32, 62))
    } catch {}
  }, [])

  useEffect(() => {
    if (!bootstrap || !selectedSession) {
      setConnection(bootstrap ? 'unavailable' : 'opening')
      return
    }
    const session = projectionRef.current.sessions.find(
      (candidate) => candidate.session_id === selectedSession,
    )
    if (!session) return

    let alive = true
    let socket: WebSocket | null = null
    let timer: number | undefined
    let attempt = 0
    let forceSnapshot =
      projectionRef.current.snapshot?.session.session_id !== selectedSession ||
      projectionRef.current.snapshotRequired

    const open = () => {
      if (!alive) return
      const current = projectionRef.current
      const cursor =
        !forceSnapshot && current.generation !== null && current.sequence !== null
          ? { generation: current.generation, sequence: current.sequence }
          : undefined
      setConnection(attempt === 0 ? 'opening' : 'reconnecting')
      socket = new WebSocket(
        eventSocketUrl(window.location.origin, session.profile_id, selectedSession, cursor),
      )
      socket.addEventListener('message', (event) => {
        if (!alive || typeof event.data !== 'string') return
        try {
          setProjection((currentProjection) => {
            const next = applyWireMessage(currentProjection, event.data)
            projectionRef.current = next
            if (next.snapshotRequired) {
              forceSnapshot = true
              window.setTimeout(() => socket?.close(4001, 'snapshot-required'), 0)
            } else if (next.snapshot?.session.session_id === selectedSession) {
              forceSnapshot = false
              attempt = 0
              setConnection('connected')
              setNotice(null)
            }
            return next
          })
        } catch {
          forceSnapshot = true
          setNotice('The conversation changed shape. Keith is loading a fresh authoritative copy.')
          socket?.close(4001, 'projection-error')
        }
      })
      socket.addEventListener('close', () => {
        if (!alive) return
        setConnection('reconnecting')
        const delay = RECONNECT_DELAYS[Math.min(attempt, RECONNECT_DELAYS.length - 1)]!
        attempt += 1
        timer = window.setTimeout(open, delay)
      })
      socket.addEventListener('error', () => setConnection('reconnecting'))
    }
    open()
    return () => {
      alive = false
      if (timer !== undefined) window.clearTimeout(timer)
      socket?.close(1000, 'session changed')
    }
  }, [bootstrap, selectedSession])

  const selectedProfile = useMemo(() => {
    if (!bootstrap) return null
    const profileId = projection.sessions.find(
      (session) => session.session_id === selectedSession,
    )?.profile_id
    return (
      bootstrap.profiles.find((profile) => profile.id === profileId) ??
      bootstrap.profiles.find((profile) => profile.enabled) ??
      null
    )
  }, [bootstrap, projection.sessions, selectedSession])

  const applyEncodedWireMessage = useCallback((encoded: string) => {
    setProjection((current) => {
      const next = applyWireMessage(current, encoded)
      projectionRef.current = next
      return next
    })
  }, [])

  const applyCommandResult = useCallback((result: CommandResult) => {
    applyEncodedWireMessage(
      JSON.stringify({ message: 'command_result', payload: result }),
    )
  }, [applyEncodedWireMessage])

  const runCommand = useCallback(
    async (
      sessionId: string | null,
      command: Command,
      stream = false,
    ): Promise<CommandResult | null> => {
      if (!bootstrap || !selectedProfile) return null
      const envelope = commandEnvelope(bootstrap.protocol, sessionId, command)
      if (stream) {
        setProjection((current) => {
          const next = beginLiveRun(current, envelope.command_id)
          projectionRef.current = next
          return next
        })
      }
      try {
        const result = await executeCommand(
          bootstrap,
          selectedProfile.id,
          envelope,
          stream ? applyEncodedWireMessage : undefined,
        )
        applyCommandResult(result)
        return result
      } catch (error) {
        if (stream) {
          setProjection((current) => ({ ...current, liveRun: null }))
        }
        const message = error instanceof Error ? error.message : 'Keith could not accept the request.'
        setNotice(message)
        return null
      }
    },
    [applyCommandResult, applyEncodedWireMessage, bootstrap, selectedProfile],
  )

  const createConversation = useCallback(async (): Promise<string | null> => {
    if (!selectedProfile || creating) return null
    const previousSession = selectedSession
    if (previousSession) draftBySession.current.set(previousSession, draft)
    setCreating(true)
    setSelectedSession(null)
    setDraft('')
    setProjection((current) => ({
      ...current,
      snapshot: null,
      generation: null,
      sequence: null,
      snapshotRequired: false,
      terminal: null,
    }))
    const result = await runCommand(null, {
      command: 'create_session',
      parameters: {
        profile_id: selectedProfile.id,
        workspace_id: selectedProfile.workspace_id,
        title: 'New conversation',
      },
    })
    if (!result) {
      setSelectedSession(previousSession)
      setCreating(false)
      return null
    }
    const snapshot = dataFromResult<SessionSnapshot>(result, 'snapshot')
    if (!snapshot) {
      setSelectedSession(previousSession)
      setCreating(false)
      return null
    }
    setSelectedSession(snapshot.session.session_id)
    setDraft('')
    setSheet(null)
    setSidebarOpen(false)
    setCreating(false)
    return snapshot.session.session_id
  }, [creating, draft, runCommand, selectedProfile, selectedSession])

  const selectConversation = useCallback((sessionId: string) => {
    if (selectedSession) draftBySession.current.set(selectedSession, draft)
    setSelectedSession(sessionId)
    setDraft(draftBySession.current.get(sessionId) ?? '')
    setProjection((current) => ({
      ...current,
      snapshot:
        current.snapshot?.session.session_id === sessionId ? current.snapshot : null,
      generation: current.snapshot?.session.session_id === sessionId ? current.generation : null,
      sequence: current.snapshot?.session.session_id === sessionId ? current.sequence : null,
      snapshotRequired: true,
      terminal: null,
    }))
    setSheet(null)
    setSidebarOpen(false)
    setNotice(null)
  }, [draft, selectedSession])

  const submitPrompt = useCallback(
    async (delivery: 'immediate' | 'next_turn_boundary' = 'immediate') => {
      const text = draft.trim()
      if (!text || sending || creating) return
      setSending(true)
      setNotice(null)
      setPendingPrompt(text)
      setDraft('')
      let sessionId = selectedSession
      if (!sessionId) sessionId = await createConversation()
      if (!sessionId) {
        setPendingPrompt(null)
        setDraft(text)
        setSending(false)
        return
      }
      const result = await runCommand(sessionId, {
        command: 'submit_prompt',
        parameters: {
          session_id: sessionId,
          text,
          artifacts: [],
          delivery,
          reply_route: null,
        },
      }, true)
      if (result) {
        setLastPrompt(text)
        setDraft('')
        draftBySession.current.set(sessionId, '')
      } else {
        setDraft(text)
        draftBySession.current.set(sessionId, text)
      }
      setPendingPrompt(null)
      setSending(false)
    },
    [createConversation, creating, draft, runCommand, selectedSession, sending],
  )

  const steer = useCallback(async () => {
    if (!selectedSession || !draft.trim() || sending) return
    setSending(true)
    const result = await runCommand(selectedSession, {
      command: 'steer',
      parameters: {
        session_id: selectedSession,
        text: draft.trim(),
        delivery: 'next_turn_boundary',
      },
    })
    if (result) setDraft('')
    setSending(false)
  }, [draft, runCommand, selectedSession, sending])

  const stop = useCallback(async () => {
    if (!selectedSession) return
    await runCommand(selectedSession, {
      command: 'cancel',
      parameters: { target: 'session', id: selectedSession },
    })
  }, [runCommand, selectedSession])

  const retry = useCallback(async () => {
    if (!selectedSession || !lastPrompt) return
    setDraft(lastPrompt)
    requestAnimationFrame(() => document.querySelector<HTMLTextAreaElement>('#keith-composer')?.focus())
  }, [lastPrompt, selectedSession])

  const resolveConfirmation = useCallback(
    async (confirmationId: string, allow: boolean) => {
      if (!selectedSession) return
      await runCommand(selectedSession, {
        command: 'resolve_confirmation',
        parameters: {
          confirmation_id: confirmationId,
          decision: allow ? 'allow_once' : 'deny',
        },
      })
    },
    [runCommand, selectedSession],
  )

  const queryMemory = useCallback(
    async (query: string) => {
      if (!selectedProfile || !query.trim()) return
      const result = await runCommand(null, {
        command: 'query_memory',
        parameters: { profile_id: selectedProfile.id, query: query.trim(), limit: 20 },
      })
      setMemoryResults(result ? dataFromResult<MemoryResult[]>(result, 'memory') ?? [] : [])
    },
    [runCommand, selectedProfile],
  )

  const resize = (event: ReactPointerEvent<HTMLDivElement>) => {
    const bounds = rootRef.current?.getBoundingClientRect()
    if (!bounds) return
    const value = clamp(((event.clientX - bounds.left) / bounds.width) * 100, 32, 62)
    setSplitRatio(value)
    try {
      window.localStorage.setItem(SPLIT_RATIO_KEY, String(value))
    } catch {}
  }

  if (auth === 'loading') return <LoadingScreen />
  if (auth === 'login') return <LoginScreen />
  if (auth === 'error' || !bootstrap) return <UnavailableScreen onRetry={loadBootstrap} />

  const snapshot = projection.snapshot
  const active = Boolean(snapshot?.active_action || projection.liveRun)
  const hasWork = Boolean(snapshot)

  return (
    <main
      ref={rootRef}
      id="main-content"
      className="keith-shell neo-matte-theme"
      data-workspace-open={workspaceOpen ? '' : undefined}
    >
      <Sidebar
        open={sidebarOpen}
        sessions={projection.sessions}
        selected={selectedSession}
        busy={creating}
        onClose={() => setSidebarOpen(false)}
        onSelect={selectConversation}
        onNew={() => void createConversation()}
        onOpenSheet={(next) => {
          setSheet(next)
          setSidebarOpen(false)
        }}
      />

      <section
        className={`keith-narration ${fullscreenWork ? 'is-concealed' : ''}`}
        style={workspaceOpen && !fullscreenWork ? { flexBasis: `${splitRatio}%` } : undefined}
      >
        <TopBar
          connection={connection}
          active={active}
          hasWork={hasWork}
          workspaceOpen={workspaceOpen}
          busy={creating}
          onMenu={() => setSidebarOpen(true)}
          onNew={() => void createConversation()}
          onStop={() => void stop()}
          onWork={() => setWorkspaceOpen((value) => !value)}
        />
        <Conversation
          snapshot={snapshot}
          pendingPrompt={pendingPrompt}
          liveRun={projection.liveRun}
          draft={draft}
          sending={sending || creating}
          connection={connection}
          notice={notice}
          active={active}
          workspaceOpen={workspaceOpen}
          onDraft={(value) => {
            setDraft(value)
            if (selectedSession) draftBySession.current.set(selectedSession, value)
          }}
          onSubmit={() => void submitPrompt()}
          onSteer={() => void steer()}
          onQueue={() => void submitPrompt('next_turn_boundary')}
          onStop={() => void stop()}
          onRetry={() => void retry()}
          onQuickAction={(prompt) => setDraft(prompt)}
          onResolve={(id, allow) => void resolveConfirmation(id, allow)}
        />
      </section>

      {workspaceOpen && !fullscreenWork ? (
        <div
          className="keith-resizer"
          role="separator"
          aria-label="Resize conversation and Keith's Computer"
          aria-orientation="vertical"
          aria-valuemin={32}
          aria-valuemax={62}
          aria-valuenow={Math.round(splitRatio)}
          tabIndex={0}
          onPointerDown={(event) => {
            dragging.current = true
            event.currentTarget.setPointerCapture(event.pointerId)
            resize(event)
          }}
          onPointerMove={(event) => dragging.current && resize(event)}
          onPointerUp={(event) => {
            dragging.current = false
            event.currentTarget.releasePointerCapture(event.pointerId)
          }}
          onKeyDown={(event) => {
            if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
            event.preventDefault()
            const next =
              event.key === 'Home'
                ? 32
                : event.key === 'End'
                  ? 62
                  : splitRatio + (event.key === 'ArrowLeft' ? -2 : 2)
            const bounded = clamp(next, 32, 62)
            setSplitRatio(bounded)
            try {
              window.localStorage.setItem(SPLIT_RATIO_KEY, String(bounded))
            } catch {}
          }}
        >
          <span />
        </div>
      ) : null}

      <WorkStage
        open={workspaceOpen}
        fullscreen={fullscreenWork}
        snapshot={snapshot}
        onClose={() => {
          setWorkspaceOpen(false)
          setFullscreenWork(false)
        }}
        onFullscreen={() => setFullscreenWork((value) => !value)}
      />

      <ControlSheet
        sheet={sheet}
        snapshot={snapshot}
        bootstrap={bootstrap}
        profileId={selectedProfile?.id ?? null}
        sessionId={selectedSession}
        sessions={projection.sessions}
        memoryResults={memoryResults}
        onClose={() => setSheet(null)}
        onSelect={selectConversation}
        onNew={() => void createConversation()}
        onQueryMemory={(query) => void queryMemory(query)}
        onCommand={(command) => void runCommand(selectedSession, command)}
      />
    </main>
  )
}

function LoadingScreen() {
  return (
    <main className="entry-screen neo-matte-theme" aria-label="Loading Keith">
      <KeithMark active />
      <p>Opening Keith</p>
    </main>
  )
}

function LoginScreen() {
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    setBusy(true)
    setError(null)
    const form = new FormData(event.currentTarget)
    const response = await fetch('/auth/session', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({ password: String(form.get('password') ?? '') }),
    })
    if (response.ok) window.location.assign('/')
    else {
      setError('That password was not accepted.')
      setBusy(false)
    }
  }
  return (
    <main className="login-page neo-matte-theme">
      <section className="login-card">
        <KeithMark />
        <p className="eyebrow">Your personal intelligence</p>
        <h1>Welcome back</h1>
        <p className="muted">Sign in to continue with Keith.</p>
        <form onSubmit={submit}>
          <label htmlFor="password">Password</label>
          <input
            id="password"
            name="password"
            type="password"
            autoComplete="current-password"
            required
            maxLength={4096}
            autoFocus
          />
          {error ? <p className="form-error">{error}</p> : null}
          <button type="submit" disabled={busy}>{busy ? 'Signing in…' : 'Continue'}</button>
        </form>
      </section>
    </main>
  )
}

function UnavailableScreen({ onRetry }: { onRetry: () => void }) {
  return (
    <main className="entry-screen neo-matte-theme">
      <Warning size={24} />
      <h1>Keith is unavailable</h1>
      <p>The interface could not reach the local runtime.</p>
      <button className="secondary-button" onClick={onRetry}><Refresh size={16} /> Try again</button>
    </main>
  )
}

function Sidebar({
  open,
  sessions,
  selected,
  busy,
  onClose,
  onSelect,
  onNew,
  onOpenSheet,
}: {
  open: boolean
  sessions: SessionSummary[]
  selected: string | null
  busy: boolean
  onClose: () => void
  onSelect: (id: string) => void
  onNew: () => void
  onOpenSheet: (sheet: Exclude<SheetName, null>) => void
}) {
  return (
    <>
      {open ? <button className="mobile-scrim" aria-label="Close menu" onClick={onClose} /> : null}
      <aside className={`keith-sidebar ${open ? 'is-open' : ''}`} aria-label="Keith navigation">
        <div className="sidebar-brand">
          <KeithMark />
          <strong>Keith</strong>
          <button className="icon-button mobile-only" onClick={onClose} aria-label="Close menu"><X /></button>
        </div>
        <button className="new-chat" onClick={onNew} disabled={busy}><Plus size={17} /> {busy ? 'Creating…' : 'New conversation'}</button>
        <div className="sidebar-label">Conversations</div>
        <nav className="conversation-list" aria-label="Conversations">
          {sessions.length === 0 ? <p className="sidebar-empty">Your conversations will appear here.</p> : null}
          {sessions.map((session) => (
            <button
              key={session.session_id}
              className={session.session_id === selected ? 'is-active' : ''}
              data-session-id={session.session_id}
              onClick={() => onSelect(session.session_id)}
            >
              <Chat size={16} />
              <span>
                <strong>{session.title || 'New conversation'}</strong>
                <small>{friendlyState(session.state)}</small>
              </span>
              <More size={15} />
            </button>
          ))}
        </nav>
        <nav className="secondary-nav" aria-label="Keith controls">
          <button onClick={() => onOpenSheet('sessions')}><Archive /> Sessions</button>
          <button onClick={() => onOpenSheet('work')}><Activity /> Work</button>
          <button onClick={() => onOpenSheet('memory')}><Memory /> Memory</button>
          <button onClick={() => onOpenSheet('schedule')}><Calendar /> Schedules</button>
          <button onClick={() => onOpenSheet('settings')}><Settings /> Settings</button>
        </nav>
      </aside>
    </>
  )
}

function TopBar({
  connection,
  active,
  hasWork,
  workspaceOpen,
  busy,
  onMenu,
  onNew,
  onStop,
  onWork,
}: {
  connection: ConnectionState
  active: boolean
  hasWork: boolean
  workspaceOpen: boolean
  busy: boolean
  onMenu: () => void
  onNew: () => void
  onStop: () => void
  onWork: () => void
}) {
  return (
    <header className="keith-topbar">
      <button className="icon-button mobile-menu" onClick={onMenu} aria-label="Open menu"><Menu /></button>
      <div className="runtime-title"><KeithMark active={active} /><span>Keith</span></div>
      <div className="topbar-actions">
        <span className={`state-pill ${connection}`}>{connectionLabel(connection, active)}</span>
        {active ? <button className="text-button danger" onClick={onStop}><Stop size={13} /> Stop</button> : null}
        <button className="icon-button" onClick={onNew} aria-label="New conversation" disabled={busy}><Plus /></button>
        <button
          className={`icon-button work-toggle ${workspaceOpen ? 'is-active' : ''}`}
          onClick={onWork}
          aria-label={workspaceOpen ? "Hide Keith's Computer" : "Show Keith's Computer"}
          disabled={!hasWork}
        ><Monitor /></button>
      </div>
    </header>
  )
}

function Conversation({
  snapshot,
  pendingPrompt,
  liveRun,
  draft,
  sending,
  connection,
  notice,
  active,
  workspaceOpen,
  onDraft,
  onSubmit,
  onSteer,
  onQueue,
  onStop,
  onRetry,
  onQuickAction,
  onResolve,
}: {
  snapshot: SessionSnapshot | null
  pendingPrompt: string | null
  liveRun: LiveRunProjection | null
  draft: string
  sending: boolean
  connection: ConnectionState
  notice: string | null
  active: boolean
  workspaceOpen: boolean
  onDraft: (value: string) => void
  onSubmit: () => void
  onSteer: () => void
  onQueue: () => void
  onStop: () => void
  onRetry: () => void
  onQuickAction: (prompt: string) => void
  onResolve: (id: string, allow: boolean) => void
}) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const pinnedToBottom = useRef(true)
  const messages = snapshot?.messages.filter(
    (message) => message.role === 'user' || message.role === 'assistant',
  ) ?? []
  const pendingVisible = Boolean(
    pendingPrompt &&
      !messages.some(
        (message) => message.role === 'user' && visibleUserText(message.text) === pendingPrompt,
      ),
  )
  const timelineMessages = pendingVisible
    ? messages.filter((message) => message.committed)
    : messages
  const activeStreamMessages = pendingVisible
    ? messages.filter((message) => !message.committed)
    : []
  useEffect(() => {
    const element = scrollRef.current
    if (element) element.scrollTop = element.scrollHeight
  }, [liveRun?.phase, liveRun?.tools.length, messages.length, messages.at(-1)?.text, pendingVisible])
  useEffect(() => {
    const frame = window.requestAnimationFrame(() => {
      const element = scrollRef.current
      if (element) element.scrollTop = element.scrollHeight
    })
    return () => window.cancelAnimationFrame(frame)
  }, [workspaceOpen])
  useEffect(() => {
    const element = scrollRef.current
    const content = element?.firstElementChild
    if (!element || !content || typeof ResizeObserver === 'undefined') return
    const trackPosition = () => {
      pinnedToBottom.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80
    }
    const observer = new ResizeObserver(() => {
      if (pinnedToBottom.current) element.scrollTop = element.scrollHeight
    })
    trackPosition()
    element.addEventListener('scroll', trackPosition, { passive: true })
    observer.observe(content)
    return () => {
      element.removeEventListener('scroll', trackPosition)
      observer.disconnect()
    }
  }, [])

  const isConversation = messages.length > 0 || Boolean(snapshot) || pendingVisible
  return (
    <div className="conversation-stage">
      {notice ? <div className="notice" role="status">{notice}</div> : null}
      {connection === 'reconnecting' ? (
        <div className="connection-notice" role="status">Connection lost — retrying from the last confirmed event.</div>
      ) : null}
      {isConversation ? (
        <div className="message-scroll" ref={scrollRef} id="conversation" aria-live="polite">
          <div className="message-column">
            {timelineMessages.map((message) => <Message key={message.message_id} message={message} />)}
            {pendingVisible && pendingPrompt ? (
              <div className="user-row pending-user" data-testid="pending-user-message">
                <div>
                  <div className="user-bubble">{pendingPrompt}</div>
                  <small>{liveRun?.phase === 'sending' ? 'Sending…' : 'Accepted'}</small>
                </div>
              </div>
            ) : null}
            {activeStreamMessages.map((message) => <Message key={message.message_id} message={message} />)}
            {liveRun ? <LiveRun run={liveRun} /> : null}
            {active && !messages.some((message) => message.role === 'assistant' && !message.committed) ? (
              liveRun ? null : <div className="assistant-row"><KeithMark active /><span className="thinking-label">Keith is working</span></div>
            ) : null}
            {snapshot?.confirmations.map((confirmation) => (
              <div className="confirmation-card" key={confirmation.confirmation_id}>
                <div><Warning size={18} /><strong>Keith needs your approval</strong></div>
                <p>{confirmation.summary}</p>
                <div className="confirmation-actions">
                  <button className="secondary-button" onClick={() => onResolve(confirmation.confirmation_id, false)}>Deny</button>
                  <button className="primary-button" onClick={() => onResolve(confirmation.confirmation_id, true)}><Check size={16} /> Allow once</button>
                </div>
              </div>
            ))}
          </div>
        </div>
      ) : (
        <div className="idle-home">
          <h1>What can I help you get done?</h1>
          <div className="idle-composer"><Composer {...{ draft, sending, active, onDraft, onSubmit, onSteer, onQueue, onStop }} /></div>
          <div className="quick-grid" aria-label="Suggested prompts">
            {QUICK_ACTIONS.map(({ title, description, prompt, icon: Icon }) => (
              <button key={title} onClick={() => onQuickAction(prompt)}>
                <span className="quick-icon"><Icon size={17} /></span>
                <strong>{title}</strong>
                <small>{description}</small>
              </button>
            ))}
          </div>
        </div>
      )}
      {isConversation ? (
        <div className="composer-dock">
          <Composer {...{ draft, sending, active, onDraft, onSubmit, onSteer, onQueue, onStop }} />
          <div className="composer-footer">
            <span>{snapshot ? `${snapshot.messages.filter((message) => message.role === 'user').length} turns` : ''}</span>
            <span>Keith is AI and can make mistakes. Double-check important information.</span>
            {snapshot?.terminal?.status === 'failed' ? <button onClick={onRetry}>Retry last message</button> : null}
          </div>
        </div>
      ) : null}
    </div>
  )
}

function LiveRun({ run }: { run: LiveRunProjection }) {
  const labels: Record<LiveRunProjection['phase'], string> = {
    sending: 'Sending to Keith',
    accepted: 'Request accepted',
    thinking: run.turn && run.turn > 1 ? `Thinking · step ${run.turn}` : 'Thinking',
    responding: 'Writing a response',
    using_tools: 'Using tools',
    finalizing: 'Finalizing',
  }
  return (
    <section className="live-run" aria-live="polite" aria-label="Keith live activity">
      <div className="live-run-heading">
        <span className="status-dot running" />
        <strong>{labels[run.phase]}</strong>
        {run.detail ? <span>{friendlyActivity(run.detail)}</span> : null}
      </div>
      {run.tools.length ? (
        <div className="live-tool-list">
          {run.tools.map((tool) => (
            <div className="live-tool" key={tool.tool_call_id} data-state={tool.state}>
              <Tools size={14} />
              <span>{friendlyActivity(tool.tool || 'Keith tool')}</span>
              <small>{friendlyState(tool.state)}</small>
            </div>
          ))}
        </div>
      ) : null}
    </section>
  )
}

function friendlyActivity(value: string): string {
  return value.replaceAll('_', ' ').replace(/\b\w/g, (letter) => letter.toUpperCase())
}

function Message({ message }: { message: MessageProjection }) {
  const [copied, setCopied] = useState(false)
  const text = message.role === 'user' ? visibleUserText(message.text) : message.text
  const commentary = message.role === 'assistant' && message.committed && !message.final_id
  const copy = async () => {
    await navigator.clipboard?.writeText(text)
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1_400)
  }
  if (message.role === 'user') {
    return <div className="user-row"><div className="user-bubble">{text}</div></div>
  }
  return (
    <article
      className={`assistant-row ${commentary ? 'assistant-commentary' : ''}`}
      data-committed={message.committed ? '' : undefined}
      data-kind={commentary ? 'commentary' : 'response'}
    >
      <KeithMark active={!message.committed} />
      <div className="assistant-content">
        {commentary ? <span className="commentary-label">Progress</span> : null}
        <Streamdown
          mode={message.committed ? 'static' : 'streaming'}
          parseIncompleteMarkdown={!message.committed}
          controls={{ code: true, table: true, mermaid: false }}
        >{text}</Streamdown>
        {message.committed ? (
          <div className="message-actions"><button onClick={() => void copy()}>{copied ? <Check size={15} /> : <Copy size={15} />} {copied ? 'Copied' : 'Copy'}</button></div>
        ) : <span className="stream-caret" />}
      </div>
    </article>
  )
}

function Composer({
  draft,
  sending,
  active,
  onDraft,
  onSubmit,
  onSteer,
  onQueue,
  onStop,
}: {
  draft: string
  sending: boolean
  active: boolean
  onDraft: (value: string) => void
  onSubmit: () => void
  onSteer: () => void
  onQueue: () => void
  onStop: () => void
}) {
  const input = useRef<HTMLTextAreaElement>(null)
  useEffect(() => {
    const element = input.current
    if (!element) return
    element.style.height = '0px'
    element.style.height = `${Math.min(160, element.scrollHeight)}px`
  }, [draft])
  return (
    <div className="keith-composer" data-layout={draft.includes('\n') ? 'expanded' : 'compact'}>
      {active && draft.trim() ? (
        <div className="live-actions">
          <button onClick={onSteer}><strong>Steer current work</strong><span>Apply this now</span></button>
          <button onClick={onQueue}><strong>Queue after this turn</strong><span>Preserve the current work</span></button>
          <button onClick={onStop}><strong>Stop current work</strong><span>Then send when ready</span></button>
        </div>
      ) : null}
      <div className="composer-row">
        <button className="composer-tool" aria-label="Keith tools" title="Keith chooses the right tools"><Tools size={18} /></button>
        <textarea
          ref={input}
          id="keith-composer"
          rows={1}
          value={draft}
          onChange={(event) => onDraft(boundUtf8(event.target.value, 65_536))}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.shiftKey) {
              event.preventDefault()
              if (active) onSteer()
              else onSubmit()
            }
          }}
          placeholder={active ? 'Write an instruction for the active work…' : 'Ask Keith'}
          aria-label="Ask Keith"
          disabled={sending}
        />
        {active && !draft.trim() ? (
          <button className="composer-send" onClick={onStop} aria-label="Stop Keith"><Stop size={14} /></button>
        ) : (
          <button className="composer-send" onClick={active ? onSteer : onSubmit} disabled={!draft.trim() || sending} aria-label={active ? 'Steer Keith' : 'Send'}><ArrowUp size={18} /></button>
        )}
      </div>
    </div>
  )
}

function WorkStage({
  open,
  fullscreen,
  snapshot,
  onClose,
  onFullscreen,
}: {
  open: boolean
  fullscreen: boolean
  snapshot: SessionSnapshot | null
  onClose: () => void
  onFullscreen: () => void
}) {
  if (!open) return null
  const items = [
    ...snapshot?.tools.map((tool) => ({ id: tool.tool_call_id, kind: 'Tool', title: tool.tool || 'Keith tool', state: tool.state })) ?? [],
    ...snapshot?.goals.map((goal, index) => ({ id: String(goal.goal_id ?? `goal-${index}`), kind: 'Goal', title: String(goal.objective ?? 'Goal'), state: String(goal.state ?? '') })) ?? [],
    ...snapshot?.children.map((child, index) => ({ id: String(child.child_id ?? `child-${index}`), kind: 'Agent', title: String(child.objective ?? 'Delegated work'), state: String(child.state ?? '') })) ?? [],
    ...snapshot?.waits.map((wait, index) => ({ id: String(wait.wait_id ?? `wait-${index}`), kind: 'Wait', title: String(wait.reason ?? 'Waiting'), state: String(wait.state ?? '') })) ?? [],
  ]
  return (
    <aside className={`work-stage ${fullscreen ? 'is-fullscreen' : ''}`} aria-label="Keith's Computer">
      <header>
        <div><Monitor size={18} /><span><strong>Keith’s Computer</strong><small>{snapshot?.active_action ? 'Working' : 'Ready'}</small></span></div>
        <div>
          <button className="icon-button desktop-only" onClick={onFullscreen} aria-label={fullscreen ? 'Exit full screen' : 'Full screen'}><Monitor size={17} /></button>
          <button className="icon-button" onClick={onClose} aria-label="Close Keith's Computer"><X /></button>
        </div>
      </header>
      <div className="work-body">
        {snapshot?.active_action ? (
          <section className="current-work">
            <div className="work-kicker"><span className="status-dot running" /> Current work</div>
            <h2>{snapshot.active_action.source || 'Keith is working'}</h2>
            <p>Authoritative state: {snapshot.active_action.state}</p>
          </section>
        ) : (
          <section className="computer-empty"><KeithMark /><h2>Keith’s workspace is ready</h2><p>Tool calls, goals, child agents, waits, and durable outputs appear here while the conversation stays mounted.</p></section>
        )}
        {items.length ? <div className="work-feed">{items.map((item) => (
          <article key={item.id}>
            <span className="work-kind">{item.kind}</span>
            <strong>{item.title}</strong>
            <small>{friendlyState(item.state)}</small>
          </article>
        ))}</div> : null}
        {snapshot?.terminal ? (
          <section className={`terminal-card ${snapshot.terminal.status}`}>
            {snapshot.terminal.status === 'completed' ? <CheckCircle size={18} /> : <Warning size={18} />}
            <div><strong>Turn {friendlyState(snapshot.terminal.status)}</strong><p>{snapshot.terminal.detail || 'Durable final state recorded.'}</p></div>
          </section>
        ) : null}
      </div>
    </aside>
  )
}

function ControlSheet({
  sheet,
  snapshot,
  bootstrap,
  profileId,
  sessionId,
  sessions,
  memoryResults,
  onClose,
  onSelect,
  onNew,
  onQueryMemory,
  onCommand,
}: {
  sheet: SheetName
  snapshot: SessionSnapshot | null
  bootstrap: BootstrapData
  profileId: string | null
  sessionId: string | null
  sessions: SessionSummary[]
  memoryResults: MemoryResult[]
  onClose: () => void
  onSelect: (id: string) => void
  onNew: () => void
  onQueryMemory: (query: string) => void
  onCommand: (command: Command) => void
}) {
  if (!sheet) return null
  const titles: Record<Exclude<SheetName, null>, string> = {
    sessions: 'Sessions',
    work: 'Keith’s work',
    memory: 'Memory',
    schedule: 'Schedules',
    settings: 'Settings',
  }
  return (
    <div className="sheet-layer" role="dialog" aria-modal="true" aria-label={titles[sheet]}>
      <button className="sheet-scrim" onClick={onClose} aria-label="Close" />
      <aside className="control-sheet">
        <header><div><span className="eyebrow">Keith</span><h2>{titles[sheet]}</h2></div><button className="icon-button" onClick={onClose} aria-label="Close"><X /></button></header>
        <div className="sheet-body">
          {sheet === 'sessions' ? <SessionsPanel sessions={sessions} selected={sessionId} onSelect={onSelect} onNew={onNew} /> : null}
          {sheet === 'work' ? <WorkPanel snapshot={snapshot} onCommand={onCommand} /> : null}
          {sheet === 'memory' ? <MemoryPanel results={memoryResults} onSearch={onQueryMemory} /> : null}
          {sheet === 'schedule' ? <SchedulePanel profileId={profileId} sessionId={sessionId} snapshot={snapshot} onCommand={onCommand} /> : null}
          {sheet === 'settings' ? <SettingsPanel bootstrap={bootstrap} profileId={profileId} sessionId={sessionId} snapshot={snapshot} onCommand={onCommand} /> : null}
        </div>
      </aside>
    </div>
  )
}

function SessionsPanel({ sessions, selected, onSelect, onNew }: { sessions: SessionSummary[]; selected: string | null; onSelect: (id: string) => void; onNew: () => void }) {
  return <section><button className="primary-button wide" onClick={onNew}><Plus size={16} /> New conversation</button><div className="panel-list">{sessions.map((session) => <button key={session.session_id} className={session.session_id === selected ? 'is-active' : ''} onClick={() => onSelect(session.session_id)}><Chat /><span><strong>{session.title || 'New conversation'}</strong><small>{friendlyState(session.state)}</small></span></button>)}</div></section>
}

function WorkPanel({ snapshot, onCommand }: { snapshot: SessionSnapshot | null; onCommand: (command: Command) => void }) {
  const createGoal = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (!snapshot) return
    const data = new FormData(event.currentTarget)
    const objective = String(data.get('objective') ?? '').trim()
    if (!objective) return
    onCommand({ command: 'create_goal', parameters: { session_id: snapshot.session.session_id, objective, limits: { max_turns: null, max_tokens: null, deadline: null } } })
    event.currentTarget.reset()
  }
  const createChild = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (!snapshot) return
    const data = new FormData(event.currentTarget)
    const objective = String(data.get('objective') ?? '').trim()
    if (!objective) return
    onCommand({ command: 'create_child', parameters: { parent_session_id: snapshot.session.session_id, objective, workspace_mode: 'isolated_copy', limits: { max_turns: null, max_tokens: null, deadline: null } } })
    event.currentTarget.reset()
  }
  return <div className="stacked-panels"><FormCard icon={<Goal />} title="Create a goal" name="objective" placeholder="Outcome Keith should own" onSubmit={createGoal} /><FormCard icon={<Agent />} title="Delegate to a child agent" name="objective" placeholder="Bounded delegated objective" onSubmit={createChild} /><ProjectionList title="Goals" items={snapshot?.goals ?? []} /><ProjectionList title="Children" items={snapshot?.children ?? []} /><ProjectionList title="Commitments" items={snapshot?.commitments ?? []} /><ProjectionList title="Waits" items={snapshot?.waits ?? []} /></div>
}

function MemoryPanel({ results, onSearch }: { results: MemoryResult[]; onSearch: (query: string) => void }) {
  const submit = (event: FormEvent<HTMLFormElement>) => { event.preventDefault(); const query = String(new FormData(event.currentTarget).get('query') ?? ''); onSearch(query) }
  return <section><form className="search-form" onSubmit={submit}><Search size={17} /><input name="query" placeholder="Search Keith’s saved context" autoComplete="off" /><button type="submit">Search</button></form><div className="memory-results">{results.length ? results.map((result, index) => <article key={`${result.source}-${index}`}><span>{result.source}</span><p>{result.excerpt}</p><small>{Math.round(result.score_micros / 10_000)}% match</small></article>) : <EmptyPanel icon={<Memory size={22} />} title="Search durable memory" copy="Keith returns cited profile-scoped memory without exposing credentials or hidden state." />}</div></section>
}

function SchedulePanel({ profileId, sessionId, snapshot, onCommand }: { profileId: string | null; sessionId: string | null; snapshot: SessionSnapshot | null; onCommand: (command: Command) => void }) {
  const submit = (event: FormEvent<HTMLFormElement>) => { event.preventDefault(); if (!profileId) return; const data = new FormData(event.currentTarget); const prompt = String(data.get('prompt') ?? '').trim(); const seconds = Number(data.get('seconds')); if (!prompt || !Number.isFinite(seconds) || seconds < 60) return; onCommand({ command: 'create_schedule', parameters: { profile_id: profileId, session_id: sessionId, expression: { kind: 'interval_seconds', value: seconds }, time_zone: Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC', prompt, reply_route: null } }); event.currentTarget.reset() }
  return <section><form className="panel-form" onSubmit={submit}><label>What should Keith do?<textarea name="prompt" required placeholder="Review the workspace and summarize changes" /></label><label>Repeat every<input name="seconds" type="number" min={60} defaultValue={3600} required /><span className="field-suffix">seconds</span></label><button className="primary-button" type="submit"><Calendar size={16} /> Create schedule</button></form><ProjectionList title="Current schedules" items={snapshot?.schedules ?? []} /></section>
}

function SettingsPanel({ bootstrap, profileId, sessionId, snapshot, onCommand }: { bootstrap: BootstrapData; profileId: string | null; sessionId: string | null; snapshot: SessionSnapshot | null; onCommand: (command: Command) => void }) {
  const model = (event: FormEvent<HTMLFormElement>) => { event.preventDefault(); if (!sessionId) return; const data = new FormData(event.currentTarget); onCommand({ command: 'select_model', parameters: { session_id: sessionId, provider: String(data.get('provider') ?? '').trim(), model: String(data.get('model') ?? '').trim() } }) }
  const background = (event: FormEvent<HTMLFormElement>) => { event.preventDefault(); if (!profileId) return; const mode = String(new FormData(event.currentTarget).get('mode') ?? 'disabled'); onCommand({ command: 'set_background_control', parameters: { profile_id: profileId, mode, pause_until: null } }) }
  const branchPoint = snapshot?.messages.toReversed().find((message) => message.final_id)?.final_id
  return <div className="stacked-panels">{sessionId ? <section className="settings-section"><h3>Conversation</h3><p className="muted">Resume durable work or branch from the latest committed answer without rewriting history.</p><div className="settings-actions"><button className="secondary-button" onClick={() => onCommand({ command: 'resume_session', parameters: { session_id: sessionId } })}><Refresh size={16} /> Resume</button><button className="secondary-button" disabled={!branchPoint} onClick={() => branchPoint && onCommand({ command: 'branch_session', parameters: { session_id: sessionId, parent_entry_id: branchPoint, label: null } })}><Chat size={16} /> Branch</button></div></section> : null}<section className="settings-section"><h3>Model</h3><form className="panel-form compact" onSubmit={model}><label>Provider<input name="provider" placeholder="xiaomi" required /></label><label>Model<input name="model" placeholder="mimo-v2.5-pro" required /></label><button className="primary-button" type="submit">Use model</button></form></section><section className="settings-section"><h3>Background work</h3><form className="panel-form compact" onSubmit={background}><label>Mode<select name="mode" defaultValue="disabled"><option value="disabled">Disabled</option><option value="suggest">Suggest</option><option value="confirm_selected">Confirm selected</option><option value="bounded">Bounded</option></select></label><button className="primary-button" type="submit">Save mode</button></form></section>{profileId ? <section className="settings-section"><h3>Provider credential</h3><p className="muted">Write-only. The value is submitted directly to Keith and never saved in browser storage.</p><form className="panel-form compact" method="post" action={`/api/profiles/${encodeURIComponent(profileId)}/credentials`}><input type="hidden" name="csrf" value={bootstrap.csrf} /><label>Provider<input name="provider" required placeholder="xiaomi" /></label><label>Credential name<input name="name" required placeholder="api-key" /></label><label>Secret<input name="secret" type="password" required autoComplete="off" /></label><button className="primary-button" type="submit">Save credential</button></form></section> : null}{sessionId ? <section className="settings-section"><h3>Portable export</h3><button className="secondary-button" onClick={() => onCommand({ command: 'export', parameters: { session_id: sessionId, format: 'portable_bundle', include_artifacts: true } })}><Download size={16} /> Prepare export</button></section> : null}</div>
}

function FormCard({ icon, title, name, placeholder, onSubmit }: { icon: ReactNode; title: string; name: string; placeholder: string; onSubmit: (event: FormEvent<HTMLFormElement>) => void }) {
  return <form className="form-card" onSubmit={onSubmit}><div>{icon}<strong>{title}</strong></div><textarea name={name} placeholder={placeholder} required /><button className="primary-button" type="submit">Create</button></form>
}

function ProjectionList({ title, items }: { title: string; items: Array<Record<string, unknown>> }) {
  if (!items.length) return null
  return <section className="projection-list"><h3>{title}</h3>{items.map((item, index) => <article key={String(item.id ?? item.goal_id ?? item.child_id ?? item.job_id ?? index)}><strong>{String(item.objective ?? item.title ?? item.summary ?? item.prompt ?? title.slice(0, -1))}</strong><small>{friendlyState(String(item.state ?? item.status ?? item.change ?? ''))}</small></article>)}</section>
}

function EmptyPanel({ icon, title, copy }: { icon: ReactNode; title: string; copy: string }) {
  return <div className="empty-panel">{icon}<strong>{title}</strong><p>{copy}</p></div>
}

function KeithMark({ active = false }: { active?: boolean }) {
  return <span className={`keith-mark ${active ? 'is-active' : ''}`} aria-hidden="true"><span /><span /><span /></span>
}

function connectionLabel(connection: ConnectionState, active: boolean): string {
  if (connection === 'connected') return active ? 'Working' : 'Ready'
  if (connection === 'reconnecting') return 'Reconnecting'
  if (connection === 'opening') return 'Connecting'
  return 'Offline'
}

function friendlyState(value: string): string {
  if (!value) return 'Ready'
  return value.replaceAll('_', ' ').replace(/\b\w/g, (letter) => letter.toUpperCase())
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value))
}

function boundUtf8(value: string, maximum: number): string {
  const encoder = new TextEncoder()
  if (encoder.encode(value).byteLength <= maximum) return value
  let output = ''
  for (const character of value) {
    if (encoder.encode(output + character).byteLength > maximum) break
    output += character
  }
  return output
}
