import { For, Show, createResource } from "solid-js"
import { createBrowserProjection } from "./bridge"
import type { BootstrapData } from "./types"

export interface AppProps {
  bootstrap: BootstrapData
}

export function App(props: AppProps) {
  const [bridge] = createResource(createBrowserProjection)
  const firstSession = () => props.bootstrap.sessions[0]

  return (
    <div class="foundation-shell">
      <header class="foundation-header">
        <div>
          <p class="foundation-kicker">Personal intelligence</p>
          <h1>Keith</h1>
        </div>
        <p class="foundation-status" role="status" aria-live="polite">
          {bridge.loading ? "Connecting" : bridge.error ? "Connection unavailable" : "Ready"}
        </p>
      </header>
      <main class="foundation-main">
        <section class="foundation-welcome" aria-labelledby="welcome-title">
          <p class="foundation-kicker">Good to see you</p>
          <h2 id="welcome-title">What can I take care of?</h2>
          <p>
            Start a conversation and Keith will keep the work, decisions, and finished results
            together.
          </p>
          <Show when={firstSession()} fallback={<p>Keith is ready when you are.</p>}>
            {(session) => <p>Continue {session().title ?? "your conversation"}.</p>}
          </Show>
        </section>
        <section class="foundation-recents" aria-labelledby="recent-title">
          <h2 id="recent-title">Recent conversations</h2>
          <Show
            when={props.bootstrap.sessions.length > 0}
            fallback={<p>Ask Keith for help and your conversation will stay here.</p>}
          >
            <ul>
              <For each={props.bootstrap.sessions}>
                {(session) => <li>{session.title ?? "New conversation"}</li>}
              </For>
            </ul>
          </Show>
        </section>
      </main>
    </div>
  )
}
