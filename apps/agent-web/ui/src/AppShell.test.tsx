import { createSignal } from "solid-js"
import { render } from "solid-js/web"
import { afterEach, describe, expect, it } from "vitest"
import { AppShell, type Destination } from "./AppShell"
import type { BrowserView, SessionSummary } from "./types"

const sessions: SessionSummary[] = [
  {
    session_id: "01K00000000000000000000001",
    root_tree_id: "01K00000000000000000000002",
    profile_id: "01K00000000000000000000003",
    title: "Plan the family trip",
    state: "ready",
    updated_at: "1970-01-01T00:00:00Z"
  }
]

let dispose: (() => void) | undefined

afterEach(() => {
  dispose?.()
  dispose = undefined
  document.body.replaceChildren()
  document.documentElement.dataset.theme = "light"
})

function mount(view: BrowserView = { snapshot_required: false }) {
  const host = document.createElement("div")
  document.body.append(host)
  const [destination, setDestination] = createSignal<Destination>("home")
  const [selectedSession, setSelectedSession] = createSignal<string | undefined>(
    sessions[0]?.session_id
  )
  dispose = render(
    () => (
      <AppShell
        destination={destination}
        setDestination={setDestination}
        view={() => view}
        sessions={sessions}
        selectedSession={selectedSession}
        setSelectedSession={setSelectedSession}
        connectionLabel={() => "Not connected"}
      />
    ),
    host
  )
  return host
}

function click(button: Element | null) {
  if (!(button instanceof HTMLElement)) throw new Error("button unavailable")
  button.dispatchEvent(new MouseEvent("click", { bubbles: true }))
}

describe("consumer personal intelligence shell", () => {
  it("keeps one conversation mounted while familiar destinations change", () => {
    const host = mount()
    const conversation = host.querySelector(".conversation-pane")
    const work = [...host.querySelectorAll("nav button")].find(
      (button) => button.textContent?.trim() === "Work"
    )
    click(work ?? null)
    expect(host.textContent).toContain("Everything Keith is taking care of")
    expect(host.querySelector(".conversation-pane")).toBe(conversation)
    expect(host.querySelectorAll(".conversation-pane")).toHaveLength(1)

    const labels = [...host.querySelectorAll(".primary-navigation button")].map((button) =>
      button.textContent?.trim()
    )
    expect(labels).toEqual(["Home", "Conversation", "Work", "Your World"])
    for (const technical of ["queue", "kernel", "worker", "generation", "tool call"]) {
      expect(host.textContent?.toLowerCase()).not.toContain(technical)
    }
  })

  it("opens a dismissible mobile navigation and real conversation picker", () => {
    const host = mount()
    click(host.querySelector(".menu-button"))
    const sheet = host.querySelector("#mobile-navigation")
    expect(sheet?.getAttribute("aria-hidden")).toBe("false")
    expect(sheet?.textContent).toContain("Plan the family trip")
    click(host.querySelector(".sheet-scrim"))
    expect(sheet?.getAttribute("aria-hidden")).toBe("true")
  })

  it("renders only authoritative brief items and keeps empty states actionable", () => {
    const host = mount({
      snapshot_required: false,
      personal: {
        session_id: sessions[0]?.session_id ?? "",
        session_title: "Plan the family trip",
        presence: {
          tone: "needs_you",
          label: "Needs you",
          updated_at: "1970-01-01T00:00:00Z"
        },
        work: [],
        needs_you: [
          {
            reference: {},
            kind: "decision",
            title: "Choose the travel dates",
            state_label: "Needs your decision"
          }
        ],
        completed: [],
        upcoming: [],
        saved_context: [],
        outputs: []
      }
    })
    expect(host.textContent).toContain("Choose the travel dates")
    expect(host.textContent).toContain("Needs your decision")
    expect(host.textContent).not.toContain("Nothing needs your attention right now")
  })
})
