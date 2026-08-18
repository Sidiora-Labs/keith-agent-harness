import type { BootstrapData, ProfileSummary, SessionSummary } from "./types"

function parseArray<T>(value: string | undefined): T[] {
  if (!value) return []
  const parsed: unknown = JSON.parse(value)
  return Array.isArray(parsed) ? (parsed as T[]) : []
}

export function readBootstrap(root: HTMLElement): BootstrapData {
  return {
    csrf: root.dataset.csrf ?? "",
    profiles: parseArray<ProfileSummary>(root.dataset.profiles),
    sessions: parseArray<SessionSummary>(root.dataset.sessions)
  }
}
