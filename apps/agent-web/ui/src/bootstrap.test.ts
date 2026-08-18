import { describe, expect, it } from "vitest"
import { readBootstrap } from "./bootstrap"

describe("authenticated bootstrap", () => {
  it("reads bounded server data without browser storage", () => {
    const root = document.createElement("div")
    root.dataset.csrf = "proof"
    root.dataset.profiles = "[]"
    root.dataset.sessions = JSON.stringify([
      {
        session_id: "session",
        root_tree_id: "root",
        profile_id: "profile",
        title: "Everyday help",
        state: "ready",
        updated_at: "1970-01-01T00:00:00Z"
      }
    ])
    const bootstrap = readBootstrap(root)
    expect(bootstrap.csrf).toBe("proof")
    expect(bootstrap.sessions[0]?.title).toBe("Everyday help")
    expect(localStorage.length).toBe(0)
    expect(sessionStorage.length).toBe(0)
  })
})
