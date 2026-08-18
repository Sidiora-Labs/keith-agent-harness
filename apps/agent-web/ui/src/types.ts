export interface ProfileSummary {
  id: string
  workspace_id: string
  display_name: string
  enabled: boolean
}

export interface SessionSummary {
  session_id: string
  root_tree_id: string
  profile_id: string
  title?: string
  state: string
  updated_at: string
}

export interface BootstrapData {
  csrf: string
  profiles: ProfileSummary[]
  sessions: SessionSummary[]
}

export interface ResumeCursor {
  root_tree_id: string
  generation: number
  last_sequence: number
}

export interface PersonalPresence {
  tone: "ready" | "active" | "waiting" | "needs_you" | "complete" | "failed"
  label: string
  detail?: string
  updated_at: string
}

export interface PersonalItem {
  reference: unknown
  kind: string
  title: string
  detail?: string
  state_label: string
  occurred_at?: string
}

export interface PersonalProjection {
  session_id: string
  session_title: string
  presence: PersonalPresence
  work: PersonalItem[]
  needs_you: PersonalItem[]
  completed: PersonalItem[]
  upcoming: PersonalItem[]
  saved_context: PersonalItem[]
  outputs: PersonalItem[]
}

export interface BrowserView {
  snapshot?: Record<string, unknown>
  personal?: PersonalProjection
  resume?: ResumeCursor
  snapshot_required: boolean
}

export interface BrowserProjectionBridge {
  apply_wire_message(message: string): string
  current_view(): string
  list_sessions(profileId?: string): string
  create_session(profileId: string, workspaceId: string, title?: string): string
  attach_session(sessionId: string): string
  submit_prompt(sessionId: string, text: string): string
  steer(sessionId: string, text: string): string
  cancel(sessionId: string): string
  retry(sessionId: string): string
  branch(sessionId: string): string
  resume(sessionId: string): string
  select_model(sessionId: string, provider: string, model: string): string
  resolve_confirmation(sessionId: string, confirmationId: string, allow: boolean): string
  create_goal(sessionId: string, objective: string): string
  create_child(sessionId: string, objective: string): string
  query_memory(profileId: string, query: string): string
  create_schedule(profileId: string, sessionId: string, prompt: string, intervalSeconds: bigint): string
  export_session(sessionId: string): string
  set_background(profileId: string, mode: string): string
}
