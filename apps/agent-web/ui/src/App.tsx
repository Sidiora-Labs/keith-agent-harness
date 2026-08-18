import { createEffect, createResource, createSignal } from "solid-js"
import { AppShell, type Destination } from "./AppShell"
import { createBrowserProjection } from "./bridge"
import { createKeithConnection } from "./connection"
import type { BootstrapData, BrowserView, SessionSummary } from "./types"

export interface AppProps {
  bootstrap: BootstrapData
}

const emptyView: BrowserView = { snapshot_required: false }

export function App(props: AppProps) {
  const [destination, setDestination] = createSignal<Destination>("home")
  const [selectedSession, setSelectedSession] = createSignal(
    props.bootstrap.sessions[0]?.session_id
  )
  const [sessions, setSessions] = createSignal<SessionSummary[]>(props.bootstrap.sessions)
  const [view, setView] = createSignal<BrowserView>(emptyView)
  const [bridge] = createResource(createBrowserProjection)

  const controller = createKeithConnection({
    bootstrap: props.bootstrap,
    bridge: () => bridge(),
    view,
    setView,
    sessions,
    setSessions,
    selectedSession,
    setSelectedSession
  })

  createEffect(() => {
    const projection = bridge()
    if (!projection || selectedSession()) return
    setView(JSON.parse(projection.current_view()) as BrowserView)
  })

  const connectionLabel = () => {
    if (bridge.loading || controller.connectionState() === "opening") return "Opening Keith"
    if (bridge.error) return "Keith is unavailable"
    if (controller.connectionState() === "reconnecting") return "Reconnecting"
    return view().personal?.presence.label ?? "Not connected"
  }

  return (
    <AppShell
      destination={destination}
      setDestination={setDestination}
      view={view}
      sessions={sessions()}
      selectedSession={selectedSession}
      setSelectedSession={controller.selectSession}
      connectionLabel={connectionLabel}
      controller={controller}
    />
  )
}
