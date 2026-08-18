import { createVirtualizer } from "@tanstack/solid-virtual"
import { For, Show, createEffect, createMemo, createSignal, type Accessor } from "solid-js"
import type { KeithController } from "./connection"
import type {
  BrowserMessage,
  BrowserView,
  PersonalItem,
  PersonalProjection,
  SessionSummary
} from "./types"

export type Destination = "home" | "conversation" | "work" | "world" | "settings"
export type ThemeMode = "system" | "light" | "dark"

export interface ShellProps {
  destination: Accessor<Destination>
  setDestination: (destination: Destination) => void
  view: Accessor<BrowserView>
  sessions: SessionSummary[]
  selectedSession: Accessor<string | undefined>
  setSelectedSession: (session: string) => void
  connectionLabel: Accessor<string>
  controller: KeithController
}

const destinations: Array<{ id: Exclude<Destination, "settings">; label: string }> = [
  { id: "home", label: "Home" },
  { id: "conversation", label: "Conversation" },
  { id: "work", label: "Work" },
  { id: "world", label: "Your World" }
]

export function AppShell(props: ShellProps) {
  const [mobileMenuOpen, setMobileMenuOpen] = createSignal(false)
  const choose = (destination: Destination) => {
    props.setDestination(destination)
    setMobileMenuOpen(false)
  }
  const selected = createMemo(() =>
    props.sessions.find((session) => session.session_id === props.selectedSession())
  )

  return (
    <div class="keith-shell" data-destination={props.destination()}>
      <a class="skip-link" href="#main-content">
        Skip to main content
      </a>
      <header class="mobile-header">
        <button
          class="quiet-button menu-button"
          type="button"
          aria-expanded={mobileMenuOpen()}
          aria-controls="mobile-navigation"
          onClick={() => setMobileMenuOpen(true)}
        >
          Menu
        </button>
        <span class="wordmark">Keith</span>
        <button class="quiet-button" type="button" onClick={() => choose("conversation")}>
          Message
        </button>
      </header>

      <aside class="primary-rail" aria-label="Keith">
        <div class="brand-lockup">
          <span class="brand-mark" aria-hidden="true">
            K
          </span>
          <div>
            <p class="wordmark">Keith</p>
            <p class="brand-caption">Personal intelligence</p>
          </div>
        </div>
        <button class="primary-action" type="button" onClick={() => choose("conversation")}>
          Message Keith
        </button>
        <nav class="primary-navigation" aria-label="Main navigation">
          <For each={destinations}>
            {(item) => (
              <button
                type="button"
                aria-current={props.destination() === item.id ? "page" : undefined}
                onClick={() => choose(item.id)}
              >
                {item.label}
              </button>
            )}
          </For>
        </nav>
        <div class="rail-footer">
          <p class="connection-line" role="status" aria-live="polite">
            {props.connectionLabel()}
          </p>
          <button
            class="settings-link"
            type="button"
            aria-current={props.destination() === "settings" ? "page" : undefined}
            onClick={() => choose("settings")}
          >
            Settings
          </button>
        </div>
      </aside>

      <Show when={mobileMenuOpen()}>
        <button
          class="sheet-scrim"
          type="button"
          aria-label="Close navigation"
          onClick={() => setMobileMenuOpen(false)}
        />
      </Show>
      <aside
        id="mobile-navigation"
        class="mobile-sheet"
        data-open={mobileMenuOpen() ? "true" : "false"}
        aria-label="Navigation and conversations"
        aria-hidden={!mobileMenuOpen()}
      >
        <div class="sheet-heading">
          <div>
            <p class="wordmark">Keith</p>
            <p class="brand-caption">Personal intelligence</p>
          </div>
          <button class="quiet-button" type="button" onClick={() => setMobileMenuOpen(false)}>
            Close
          </button>
        </div>
        <nav class="sheet-navigation" aria-label="Main navigation">
          <For each={destinations}>
            {(item) => (
              <button
                type="button"
                aria-current={props.destination() === item.id ? "page" : undefined}
                onClick={() => choose(item.id)}
              >
                {item.label}
              </button>
            )}
          </For>
          <button type="button" onClick={() => choose("settings")}>
            Settings
          </button>
        </nav>
        <SessionList
          sessions={props.sessions}
          selectedSession={props.selectedSession}
          setSelectedSession={(session) => {
            props.controller.selectSession(session)
            choose("conversation")
          }}
          newConversation={props.controller.newConversation}
        />
      </aside>

      <div class="product-stage">
        <main id="main-content" class="destination-stage" tabindex="-1">
          <Show when={props.destination() === "home"}>
            <HomeView
              personal={props.view().personal ?? undefined}
              messageKeith={() => choose("conversation")}
              controller={props.controller}
            />
          </Show>
          <Show when={props.destination() === "work"}>
            <WorkView
              personal={props.view().personal ?? undefined}
              messageKeith={() => choose("conversation")}
              controller={props.controller}
            />
          </Show>
          <Show when={props.destination() === "world"}>
            <WorldView
              personal={props.view().personal ?? undefined}
              messageKeith={() => choose("conversation")}
            />
          </Show>
          <Show when={props.destination() === "settings"}>
            <SettingsView />
          </Show>
        </main>

        <ConversationPane
          active={props.destination() === "conversation"}
          view={props.view}
          sessions={props.sessions}
          selected={selected}
          selectedSession={props.selectedSession}
          setSelectedSession={props.setSelectedSession}
          controller={props.controller}
        />
      </div>
    </div>
  )
}

function HomeView(props: {
  personal?: PersonalProjection
  messageKeith: () => void
  controller: KeithController
}) {
  const hasBrief = () =>
    Boolean(
      props.personal &&
        (props.personal.needs_you.length ||
          props.personal.work.length ||
          props.personal.completed.length ||
          props.personal.upcoming.length)
    )
  return (
    <div class="page home-page">
      <header class="hero">
        <p class="eyebrow">Your day with Keith</p>
        <h1>What can I take off your plate?</h1>
        <p class="hero-copy">
          Bring Keith a task, a decision, or something you want to stay on top of. The conversation
          and the work stay together.
        </p>
        <button class="primary-action hero-action" type="button" onClick={props.messageKeith}>
          Message Keith
        </button>
      </header>
      <Show
        when={hasBrief()}
        fallback={
          <EmptyState
            kicker="Today"
            title="Nothing needs your attention right now"
            detail="When Keith is working on something, waiting for a decision, or finishes a result, it will appear here."
            action="Start with a message"
            onAction={props.messageKeith}
          />
        }
      >
        <section class="brief-grid" aria-label="Your brief">
          <ItemSection title="Needs you" items={props.personal?.needs_you ?? []} tone="attention" />
          <ItemSection title="In progress" items={props.personal?.work ?? []} />
          <ItemSection title="Coming up" items={props.personal?.upcoming ?? []} />
          <ItemSection title="Recently finished" items={props.personal?.completed ?? []} />
        </section>
      </Show>
      <Show when={(props.personal?.outputs.length ?? 0) > 0}>
        <ItemSection
          title="Ready for you"
          items={props.personal?.outputs ?? []}
          tone="output"
          openResult={props.messageKeith}
          download={props.controller.exportResult}
        />
      </Show>
    </div>
  )
}

function WorkView(props: {
  personal?: PersonalProjection
  messageKeith: () => void
  controller: KeithController
}) {
  const total = () =>
    (props.personal?.work.length ?? 0) +
    (props.personal?.needs_you.length ?? 0) +
    (props.personal?.upcoming.length ?? 0) +
    (props.personal?.completed.length ?? 0)
  return (
    <div class="page">
      <PageHeading
        eyebrow="Work"
        title="Everything Keith is taking care of"
        detail="Active work, decisions, upcoming commitments, and finished results in one place."
      />
      <Show
        when={total() > 0}
        fallback={
          <EmptyState
            kicker="Clear slate"
            title="There is no active work yet"
            detail="Tell Keith what outcome you want. The details will stay organized here without turning your life into a project dashboard."
            action="Give Keith something to do"
            onAction={props.messageKeith}
          />
        }
      >
        <div class="stacked-sections">
          <ItemSection title="Needs you" items={props.personal?.needs_you ?? []} tone="attention" />
          <ItemSection title="Active" items={props.personal?.work ?? []} />
          <ItemSection title="Upcoming" items={props.personal?.upcoming ?? []} />
          <ItemSection title="Completed" items={props.personal?.completed ?? []} />
          <ItemSection
            title="Outputs"
            items={props.personal?.outputs ?? []}
            tone="output"
            openResult={props.messageKeith}
            download={props.controller.exportResult}
          />
        </div>
      </Show>
    </div>
  )
}

function WorldView(props: { personal?: PersonalProjection; messageKeith: () => void }) {
  return (
    <div class="page">
      <PageHeading
        eyebrow="Your World"
        title="The context that makes Keith yours"
        detail="Useful preferences, routines, and knowledge that Keith can bring into the next conversation."
      />
      <Show
        when={(props.personal?.saved_context.length ?? 0) > 0}
        fallback={
          <EmptyState
            kicker="Saved context"
            title="Keith has not saved anything here yet"
            detail="As you work together, useful context can appear here for you to inspect and correct."
            action="Talk with Keith"
            onAction={props.messageKeith}
          />
        }
      >
        <ItemSection title="Saved for next time" items={props.personal?.saved_context ?? []} />
      </Show>
    </div>
  )
}

function ConversationPane(props: {
  active: boolean
  view: Accessor<BrowserView>
  sessions: SessionSummary[]
  selected: Accessor<SessionSummary | undefined>
  selectedSession: Accessor<string | undefined>
  setSelectedSession: (session: string) => void
  controller: KeithController
}) {
  const messages = stableMessages(() => props.view().snapshot?.messages ?? [])
  return (
    <section
      class="conversation-pane"
      data-active={props.active ? "true" : "false"}
      aria-label="Conversation with Keith"
    >
      <header class="conversation-header">
        <div>
          <p class="eyebrow">Conversation</p>
          <h2>{props.selected()?.title ?? "Keith"}</h2>
        </div>
        <details class="session-menu">
          <summary>Switch</summary>
          <SessionList
            sessions={props.sessions}
            selectedSession={props.selectedSession}
            setSelectedSession={props.controller.selectSession}
            newConversation={props.controller.newConversation}
          />
        </details>
      </header>
      <ConversationHistory messages={messages} view={props.view} controller={props.controller} />
      <ConversationComposer
        selectedSession={props.selectedSession}
        controller={props.controller}
        presenceTone={() => props.view().personal?.presence.tone}
      />
    </section>
  )
}

function ConversationHistory(props: {
  messages: Accessor<BrowserMessage[]>
  view: Accessor<BrowserView>
  controller: KeithController
}) {
  let scroll: HTMLDivElement | undefined
  const [nearTail, setNearTail] = createSignal(true)
  const [newActivity, setNewActivity] = createSignal(false)
  let previousCount = 0
  const followTail = () => {
    if (!scroll) return
    scroll.scrollTop = scroll.scrollHeight
    setNearTail(true)
    setNewActivity(false)
  }
  createEffect(() => {
    const count = props.messages().length
    if (count > previousCount) {
      queueMicrotask(() => {
        if (nearTail()) followTail()
        else setNewActivity(true)
      })
    }
    previousCount = count
  })
  const approvals = () =>
    (props.view().personal?.needs_you ?? []).filter(
      (item) => item.reference.kind === "confirmation"
    )
  const outputs = () => props.view().personal?.outputs ?? []
  return (
    <div class="conversation-history">
      <div
        ref={scroll}
        class="conversation-scroll"
        role="log"
        aria-live="off"
        aria-label="Conversation history"
        onScroll={() => {
          if (!scroll) return
          const isNear = scroll.scrollHeight - scroll.scrollTop - scroll.clientHeight < 120
          setNearTail(isNear)
          if (isNear) setNewActivity(false)
        }}
      >
        <Show
          when={props.messages().length > 0}
          fallback={
            <div class="conversation-empty">
              <p class="eyebrow">A quiet place to think together</p>
              <h3>What should Keith take care of?</h3>
              <p>You can ask for an answer, hand off a task, or pick up where you left off.</p>
            </div>
          }
        >
          <Show
            when={props.messages().length > 60}
            fallback={<For each={props.messages()}>{(message) => <MessageEntry message={message} />}</For>}
          >
            <VirtualConversation messages={props.messages} scroll={() => scroll} />
          </Show>
        </Show>
        <Show when={props.view().personal?.presence.tone === "active" || props.view().personal?.presence.tone === "waiting"}>
          <div class="activity-note" role="status">
            <p>{props.view().personal?.presence.label}</p>
            <Show when={props.view().personal?.presence.detail}>
              <p>{props.view().personal?.presence.detail}</p>
            </Show>
          </div>
        </Show>
        <For each={approvals()}>
          {(item) => <ConfirmationCard item={item} controller={props.controller} />}
        </For>
        <For each={outputs()}>
          {(item) => <OutputCard item={item} controller={props.controller} open={followTail} />}
        </For>
        <Show when={props.view().snapshot?.terminal}>
          {(terminal) => (
            <section class="terminal-result" data-status={terminal().status}>
              <p class="eyebrow">Result</p>
              <h3>{terminalLabel(terminal().status)}</h3>
              <Show when={terminal().detail}>
                <p>{terminal().detail}</p>
              </Show>
            </section>
          )}
        </Show>
      </div>
      <Show when={newActivity()}>
        <button class="new-activity" type="button" onClick={followTail} aria-live="polite">
          New activity
        </button>
      </Show>
    </div>
  )
}

function VirtualConversation(props: {
  messages: Accessor<BrowserMessage[]>
  scroll: Accessor<HTMLDivElement | undefined>
}) {
  let canvas: HTMLDivElement | undefined
  const virtualizer = createVirtualizer<HTMLDivElement, HTMLElement>({
    get count() {
      return props.messages().length
    },
    getScrollElement: () => props.scroll() ?? null,
    estimateSize: () => 104,
    getItemKey: (index) => props.messages()[index]?.message_id ?? index,
    overscan: 8
  })
  createEffect(() => {
    canvas?.style.setProperty("--virtual-height", `${virtualizer.getTotalSize()}px`)
  })
  return (
    <div class="virtual-conversation" ref={canvas}>
      <For each={virtualizer.getVirtualItems()}>
        {(row) => {
          const message = () => props.messages()[row.index]
          return (
            <Show when={message()}>
              {(entry) => (
                <div
                  class="virtual-message"
                  data-index={row.index}
                  ref={(element) => {
                    element.style.setProperty("--virtual-offset", `${row.start}px`)
                    virtualizer.measureElement(element)
                  }}
                >
                  <MessageEntry message={entry()} />
                </div>
              )}
            </Show>
          )
        }}
      </For>
    </div>
  )
}

function MessageEntry(props: { message: BrowserMessage }) {
  const author = () => {
    if (props.message.role === "user") return "You"
    if (props.message.role === "tool") return "Activity"
    if (props.message.role === "system") return "Notice"
    return "Keith"
  }
  return (
    <article class="message" data-role={props.message.role}>
      <p class="message-author">{author()}</p>
      <SafeMessageContent text={props.message.text} />
      <Show when={!props.message.committed}>
        <p class="incomplete-label">Keith is responding</p>
      </Show>
    </article>
  )
}

function SafeMessageContent(props: { text: string }) {
  const blocks = createMemo(() => structuredBlocks(props.text))
  return (
    <div class="message-content">
      <For each={blocks()}>
        {(block) => (
          <Show
            when={block.kind === "code"}
            fallback={
              <Show
                when={block.kind === "list"}
                fallback={<p><SafeInline text={block.lines[0] ?? ""} /></p>}
              >
                <ul>
                  <For each={block.lines}>{(line) => <li><SafeInline text={line} /></li>}</For>
                </ul>
              </Show>
            }
          >
            <pre><code>{block.lines.join("\n")}</code></pre>
          </Show>
        )}
      </For>
    </div>
  )
}

function SafeInline(props: { text: string }) {
  return (
    <For each={linkParts(props.text)}>
      {(part) =>
        part.href ? (
          <a href={part.href} target="_blank" rel="noreferrer noopener">{part.text}</a>
        ) : (
          part.text
        )
      }
    </For>
  )
}

function ConversationComposer(props: {
  selectedSession: Accessor<string | undefined>
  controller: KeithController
  presenceTone: Accessor<string | undefined>
}) {
  const canSend = () =>
    Boolean(
      props.selectedSession() &&
        props.controller.draft().trim() &&
        !props.controller.uncertain()
    )
  return (
    <form
      class="composer"
      aria-label="Message Keith"
      onSubmit={(event) => {
        event.preventDefault()
        void props.controller.send()
      }}
    >
        <label class="visually-hidden" for="message-keith">
          Message Keith
        </label>
        <textarea
          id="message-keith"
          rows="1"
          maxlength="65536"
          placeholder="Message Keith"
          value={props.controller.draft()}
          onInput={(event) => props.controller.setDraft(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault()
              if (canSend()) void props.controller.send()
            }
          }}
          disabled={!props.selectedSession()}
          aria-describedby="composer-explanation"
        />
        <button type="submit" disabled={!canSend() || props.controller.sending()}>
          Send
        </button>
        <div class="conversation-actions" aria-label="Conversation actions">
          <button type="button" disabled={!canSend() || props.controller.sending()} onClick={() => void props.controller.steer()}>
            Guide
          </button>
          <Show when={props.presenceTone() === "active" || props.presenceTone() === "waiting"}>
            <button type="button" disabled={props.controller.sending()} onClick={() => void props.controller.stop()}>
              Stop
            </button>
          </Show>
          <Show when={!props.controller.uncertain()}>
            <details>
              <summary>More</summary>
              <div class="more-actions">
                <button type="button" onClick={() => void props.controller.retry()}>Try last message again</button>
                <button type="button" onClick={() => void props.controller.branch()}>Start a new path here</button>
                <button type="button" onClick={() => void props.controller.resume()}>Continue</button>
              </div>
            </details>
          </Show>
        </div>
        <Show when={props.controller.notice()}>
          <p class="composer-notice" role="status">{props.controller.notice()}</p>
        </Show>
        <Show when={props.controller.uncertain()}>
          <button class="safe-recovery" type="button" onClick={props.controller.recover}>
            Refresh what Keith knows
          </button>
        </Show>
        <p id="composer-explanation">
          {props.selectedSession()
            ? "Enter sends. Shift-Enter adds a new line."
            : "Choose a conversation before sending a message."}
        </p>
      </form>
  )
}

function SessionList(props: {
  sessions: SessionSummary[]
  selectedSession: Accessor<string | undefined>
  setSelectedSession: (session: string) => void
  newConversation?: () => Promise<void>
}) {
  return (
    <div class="session-list" aria-label="Conversations">
      <p class="section-label">Conversations</p>
      <Show when={props.newConversation}>
        <button type="button" class="new-conversation" onClick={() => void props.newConversation?.()}>
          New conversation
        </button>
      </Show>
      <Show
        when={props.sessions.length > 0}
        fallback={<p class="quiet-copy">Your conversations will appear here.</p>}
      >
        <For each={props.sessions}>
          {(session) => (
            <button
              type="button"
              aria-current={props.selectedSession() === session.session_id ? "true" : undefined}
              onClick={() => props.setSelectedSession(session.session_id)}
            >
              {session.title ?? "New conversation"}
            </button>
          )}
        </For>
      </Show>
    </div>
  )
}

function SettingsView() {
  const [theme, setTheme] = createSignal<ThemeMode>("system")
  const chooseTheme = (next: ThemeMode) => {
    setTheme(next)
    if (next === "system") document.documentElement.removeAttribute("data-theme")
    else document.documentElement.dataset.theme = next
  }
  return (
    <div class="page settings-page">
      <PageHeading
        eyebrow="Settings"
        title="Make Keith comfortable for you"
        detail="Appearance follows your device by default. Nothing here changes what Keith is allowed to do."
      />
      <section class="settings-group" aria-labelledby="appearance-heading">
        <h2 id="appearance-heading">Appearance</h2>
        <div class="segmented-control" role="group" aria-label="Color theme">
          <For each={["system", "light", "dark"] as ThemeMode[]}>
            {(option) => (
              <button
                type="button"
                aria-pressed={theme() === option}
                onClick={() => chooseTheme(option)}
              >
                {option[0]?.toUpperCase()}
                {option.slice(1)}
              </button>
            )}
          </For>
        </div>
      </section>
    </div>
  )
}

function ItemSection(props: {
  title: string
  items: PersonalItem[]
  tone?: "attention" | "output"
  openResult?: () => void
  download?: () => Promise<void>
}) {
  return (
    <Show when={props.items.length > 0}>
      <section class="item-section" data-tone={props.tone}>
        <h2>{props.title}</h2>
        <div class="item-list">
          <For each={props.items}>
            {(item) => (
              <article class="personal-item">
                <div>
                  <h3>{item.title}</h3>
                  <Show when={item.detail}>
                    <p>{item.detail}</p>
                  </Show>
                </div>
                <p class="state-label">{item.state_label}</p>
                <Show when={props.tone === "output"}>
                  <div class="item-actions">
                    <button type="button" onClick={props.openResult}>Open</button>
                    <button type="button" onClick={() => void props.download?.()}>Download</button>
                  </div>
                </Show>
              </article>
            )}
          </For>
        </div>
      </section>
    </Show>
  )
}

function ConfirmationCard(props: { item: PersonalItem; controller: KeithController }) {
  return (
    <section class="confirmation-card" aria-label="Keith needs your decision">
      <p class="eyebrow">Keith needs your decision</p>
      <h3>{props.item.title}</h3>
      <Show when={props.item.detail}><p>{props.item.detail}</p></Show>
      <dl>
        <div><dt>Applies to</dt><dd>Only this step in this conversation</dd></div>
        <div><dt>If allowed</dt><dd>Keith will carry out the action described above once</dd></div>
      </dl>
      <div class="decision-actions">
        <button type="button" onClick={() => void props.controller.decide(props.item, true)}>Allow once</button>
        <button type="button" onClick={() => void props.controller.decide(props.item, false)}>Deny</button>
      </div>
    </section>
  )
}

function OutputCard(props: { item: PersonalItem; controller: KeithController; open: () => void }) {
  return (
    <section class="output-card">
      <p class="eyebrow">Ready for you</p>
      <h3>{props.item.title}</h3>
      <Show when={props.item.detail}><p>{props.item.detail}</p></Show>
      <div class="output-actions">
        <button type="button" onClick={props.open}>Open</button>
        <button type="button" onClick={() => void props.controller.exportResult()}>Download</button>
        <details>
          <summary>Provenance</summary>
          <p>Created by Keith from the completed work in this conversation.</p>
        </details>
      </div>
    </section>
  )
}

type StructuredBlock = { kind: "paragraph" | "list" | "code"; lines: string[] }

export function structuredBlocks(value: string): StructuredBlock[] {
  const safe = terminalSafe(value)
  const blocks: StructuredBlock[] = []
  let inCode = false
  let pending: string[] = []
  const flush = () => {
    if (!pending.length) return
    const list = !inCode && pending.every((line) => /^\s*[-*]\s+/.test(line))
    blocks.push({
      kind: inCode ? "code" : list ? "list" : "paragraph",
      lines: list ? pending.map((line) => line.replace(/^\s*[-*]\s+/, "")) : [pending.join("\n")]
    })
    pending = []
  }
  for (const line of safe.split("\n")) {
    if (line.trimStart().startsWith("```")) {
      flush()
      inCode = !inCode
    } else if (!inCode && line.trim() === "") {
      flush()
    } else {
      pending.push(line)
    }
  }
  flush()
  return blocks.length ? blocks : [{ kind: "paragraph", lines: [""] }]
}

function linkParts(value: string): Array<{ text: string; href?: string }> {
  return value.split(/(https?:\/\/[^\s]+)/g).filter(Boolean).map((text) => {
    if (!text.startsWith("http")) return { text }
    try {
      const url = new URL(text)
      return url.protocol === "http:" || url.protocol === "https:" ? { text, href: url.href } : { text }
    } catch {
      return { text }
    }
  })
}

function terminalSafe(value: string): string {
  return [...value]
    .map((character) =>
      character < " " && character !== "\n" && character !== "\t" ? "�" : character
    )
    .join("")
}

function terminalLabel(status: "completed" | "failed" | "cancelled" | "exhausted"): string {
  if (status === "completed") return "Finished"
  if (status === "cancelled") return "Stopped"
  if (status === "exhausted") return "Keith reached the available limit"
  return "Keith could not finish"
}

function stableMessages(source: Accessor<BrowserMessage[]>): Accessor<BrowserMessage[]> {
  let previous = new Map<string, BrowserMessage>()
  return createMemo(() => {
    const next = new Map<string, BrowserMessage>()
    const messages = source().map((message) => {
      const existing = previous.get(message.message_id)
      const stable = existing && sameMessage(existing, message) ? existing : message
      next.set(message.message_id, stable)
      return stable
    })
    previous = next
    return messages
  })
}

function sameMessage(left: BrowserMessage, right: BrowserMessage): boolean {
  return left.message_id === right.message_id && left.final_id === right.final_id &&
    left.role === right.role && left.text === right.text && left.committed === right.committed
}

function PageHeading(props: { eyebrow: string; title: string; detail: string }) {
  return (
    <header class="page-heading">
      <p class="eyebrow">{props.eyebrow}</p>
      <h1>{props.title}</h1>
      <p>{props.detail}</p>
    </header>
  )
}

function EmptyState(props: {
  kicker: string
  title: string
  detail: string
  action: string
  onAction: () => void
}) {
  return (
    <section class="empty-state">
      <p class="eyebrow">{props.kicker}</p>
      <h2>{props.title}</h2>
      <p>{props.detail}</p>
      <button class="secondary-action" type="button" onClick={props.onAction}>
        {props.action}
      </button>
    </section>
  )
}
