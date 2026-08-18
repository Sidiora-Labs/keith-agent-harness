import { describe, expect, it } from "vitest"
import { eventSocketUrl, mergeSessions } from "./connection"
import type { SessionSummary } from "./types"

const session = (id: string, updated: string): SessionSummary => ({
  session_id: id,
  root_tree_id: `root-${id}`,
  profile_id: "profile",
  title: `Conversation ${id}`,
  state: "ready",
  updated_at: updated
})

describe("Keith browser connection boundaries", () => {
  it("uses an exact resume cursor without credentials or unrelated state", () => {
    const url = eventSocketUrl("https://keith.example", "profile", "session", {
      root_tree_id: "kept inside Rust",
      generation: 7,
      last_sequence: 42
    })
    expect(url).toBe("wss://keith.example/api/events/profile/session?generation=7&sequence=42")
    expect(url).not.toContain("root_tree")
    expect(url).not.toContain("token")
    expect(eventSocketUrl("http://localhost:3000", "profile", "session")).toBe(
      "ws://localhost:3000/api/events/profile/session"
    )
  })

  it("merges authoritative session summaries without duplicates", () => {
    const merged = mergeSessions(
      [session("one", "2026-01-01T00:00:00Z")],
      [session("one", "2026-01-03T00:00:00Z"), session("two", "2026-01-02T00:00:00Z")]
    )
    expect(merged.map((item) => item.session_id)).toEqual(["one", "two"])
    expect(merged[0]?.updated_at).toBe("2026-01-03T00:00:00Z")
  })
})
