import { For, Show, createMemo, createSignal, type Accessor } from "solid-js"
import type {
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
            props.setSelectedSession(session)
            choose("conversation")
          }}
        />
      </aside>

      <div class="product-stage">
        <main id="main-content" class="destination-stage" tabindex="-1">
          <Show when={props.destination() === "home"}>
            <HomeView
              personal={props.view().personal ?? undefined}
              messageKeith={() => choose("conversation")}
            />
          </Show>
          <Show when={props.destination() === "work"}>
            <WorkView
              personal={props.view().personal ?? undefined}
              messageKeith={() => choose("conversation")}
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
        />
      </div>
    </div>
  )
}

function HomeView(props: { personal?: PersonalProjection; messageKeith: () => void }) {
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
        <ItemSection title="Ready for you" items={props.personal?.outputs ?? []} tone="output" />
      </Show>
    </div>
  )
}

function WorkView(props: { personal?: PersonalProjection; messageKeith: () => void }) {
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
}) {
  const messages = () => props.view().snapshot?.messages ?? []
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
            setSelectedSession={props.setSelectedSession}
          />
        </details>
      </header>
      <div class="conversation-scroll" role="log" aria-live="polite" aria-relevant="additions text">
        <Show
          when={messages().length > 0}
          fallback={
            <div class="conversation-empty">
              <p class="eyebrow">A quiet place to think together</p>
              <h3>What should Keith take care of?</h3>
              <p>You can ask for an answer, hand off a task, or pick up where you left off.</p>
            </div>
          }
        >
          <For each={messages()}>
            {(message) => (
              <article class="message" data-role={message.role}>
                <p class="message-author">{message.role === "user" ? "You" : "Keith"}</p>
                <p>{message.text}</p>
              </article>
            )}
          </For>
        </Show>
      </div>
      <form class="composer" aria-label="Message Keith">
        <label class="visually-hidden" for="message-keith">
          Message Keith
        </label>
        <textarea
          id="message-keith"
          rows="1"
          maxlength="65536"
          placeholder="Message Keith"
          disabled={!props.selectedSession()}
          aria-describedby="composer-explanation"
        />
        <button type="submit" disabled={!props.selectedSession()}>
          Send
        </button>
        <p id="composer-explanation">
          {props.selectedSession()
            ? "Enter sends. Shift-Enter adds a new line."
            : "Choose a conversation before sending a message."}
        </p>
      </form>
    </section>
  )
}

function SessionList(props: {
  sessions: SessionSummary[]
  selectedSession: Accessor<string | undefined>
  setSelectedSession: (session: string) => void
}) {
  return (
    <div class="session-list" aria-label="Conversations">
      <p class="section-label">Conversations</p>
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
              </article>
            )}
          </For>
        </div>
      </section>
    </Show>
  )
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
