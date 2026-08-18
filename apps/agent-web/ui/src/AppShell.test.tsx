import { createSignal } from "solid-js"
import { render } from "solid-js/web"
import { afterEach, describe, expect, it } from "vitest"
import { AppShell, structuredBlocks, type Destination } from "./AppShell"
import type { KeithController } from "./connection"
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
  const [draft, setDraft] = createSignal("")
  const [notice] = createSignal<string>()
  const controller: KeithController = {
    draft,
    setDraft,
    notice,
    uncertain: () => false,
    sending: () => false,
    connectionState: () => "connected",
    selectSession: setSelectedSession,
    send: async () => {},
    steer: async () => {},
    stop: async () => {},
    retry: async () => {},
    branch: async () => {},
    resume: async () => {},
    newConversation: async () => {},
    decide: async () => {},
    exportResult: async () => {},
    recover: () => {}
  }
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
        controller={controller}
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
            reference: { kind: "confirmation", id: "internal-confirmation" },
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
    expect(host.innerHTML).not.toContain("internal-confirmation")
  })

  it("keeps drafts mounted across navigation and exposes every safe conversation action", () => {
    const host = mount()
    const textarea = host.querySelector("textarea")
    if (!(textarea instanceof HTMLTextAreaElement)) throw new Error("composer unavailable")
    textarea.value = "Keep this thought"
    textarea.dispatchEvent(new InputEvent("input", { bubbles: true }))
    const work = [...host.querySelectorAll("nav button")].find(
      (button) => button.textContent?.trim() === "Work"
    )
    click(work ?? null)
    expect((host.querySelector("textarea") as HTMLTextAreaElement).value).toBe("Keep this thought")
    for (const action of [
      "Send",
      "Guide",
      "Try last message again",
      "Start a new path here",
      "Continue",
      "New conversation"
    ]) {
      expect(host.textContent).toContain(action)
    }
  })

  it("renders structured text and control bytes without unsafe HTML", () => {
    const host = mount({
      snapshot_required: false,
      snapshot: {
        session: sessions[0]!,
        messages: [
          {
            message_id: "internal-message",
            role: "assistant",
            text: "A safe link https://example.com\n\n- one\n- two\n\n```js\nconst safe = true\n```\n\u001b<script>bad()</script>",
            committed: false
          }
        ],
        confirmations: [],
        presence: { state: "thinking", updated_at: "1970-01-01T00:00:00Z" }
      }
    })
    expect(host.querySelectorAll(".message-content li")).toHaveLength(2)
    expect(host.querySelector(".message-content code")?.textContent).toContain("const safe = true")
    expect(host.querySelector(".message-content a")?.getAttribute("href")).toBe(
      "https://example.com/"
    )
    expect(host.querySelector("script")).toBeNull()
    expect(host.textContent).toContain("�<script>bad()</script>")
    expect(host.textContent).toContain("Keith is responding")
    expect(host.innerHTML).not.toContain("internal-message")
  })

  it("publishes natural decisions and output provenance without exposing references", () => {
    const host = mount({
      snapshot_required: false,
      personal: {
        session_id: sessions[0]!.session_id,
        session_title: "Plan the family trip",
        presence: { tone: "needs_you", label: "Needs you", updated_at: "1970-01-01T00:00:00Z" },
        work: [],
        needs_you: [{
          reference: { kind: "confirmation", id: "secret-decision-reference" },
          kind: "decision",
          title: "Book the refundable fare",
          detail: "This will place the reservation.",
          state_label: "Needs your decision"
        }],
        completed: [],
        upcoming: [],
        saved_context: [],
        outputs: [{
          reference: { kind: "final", id: { turn_id: "turn", final_id: "final" } },
          kind: "output",
          title: "Trip plan",
          state_label: "Completed"
        }]
      }
    })
    for (const copy of [
      "Only this step in this conversation",
      "Allow once",
      "Deny",
      "Download",
      "Provenance"
    ]) {
      expect(host.textContent).toContain(copy)
    }
    expect(host.innerHTML).not.toContain("secret-decision-reference")
  })

  it("groups paragraphs, lists, and fenced code deterministically", () => {
    expect(structuredBlocks("First\n\n- one\n- two\n\n```\ncode\n```")).toEqual([
      { kind: "paragraph", lines: ["First"] },
      { kind: "list", lines: ["one", "two"] },
      { kind: "code", lines: ["code"] }
    ])
  })
})
