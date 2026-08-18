import { createEffect, createResource, createSignal } from "solid-js"
import { AppShell, type Destination } from "./AppShell"
import { createBrowserProjection } from "./bridge"
import type { BootstrapData, BrowserView } from "./types"

export interface AppProps {
  bootstrap: BootstrapData
}

const emptyView: BrowserView = { snapshot_required: false }

export function App(props: AppProps) {
  const [destination, setDestination] = createSignal<Destination>("home")
  const [selectedSession, setSelectedSession] = createSignal(
    props.bootstrap.sessions[0]?.session_id
  )
  const [view, setView] = createSignal<BrowserView>(emptyView)
  const [bridge] = createResource(createBrowserProjection)

  createEffect(() => {
    const projection = bridge()
    if (!projection) return
    setView(JSON.parse(projection.current_view()) as BrowserView)
  })

  const connectionLabel = () => {
    if (bridge.loading) return "Opening Keith"
    if (bridge.error) return "Keith is unavailable"
    return view().personal?.presence.label ?? "Not connected"
  }

  return (
    <AppShell
      destination={destination}
      setDestination={setDestination}
      view={view}
      sessions={props.bootstrap.sessions}
      selectedSession={selectedSession}
      setSelectedSession={setSelectedSession}
      connectionLabel={connectionLabel}
    />
  )
}
