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
  reference: PersonalReference
  kind: string
  title: string
  detail?: string
  state_label: string
  occurred_at?: string
}

export type PersonalReference =
  | { kind: "action" | "goal" | "plan" | "child" | "tool" | "commitment"; id: string }
  | { kind: "schedule" | "confirmation" | "wait" | "delivery" | "memory"; id: string }
  | { kind: "final"; id: { turn_id: string; final_id: string } }

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
  snapshot?: BrowserSnapshot | null
  personal?: PersonalProjection | null
  resume?: ResumeCursor | null
  sessions?: SessionSummary[]
  last_command?: BrowserCommandReceipt | null
  snapshot_required: boolean
}

export interface BrowserCommandReceipt {
  state: "accepted" | "updated" | "rejected"
  message: string
}

export interface BrowserMessage {
  message_id: string
  final_id?: string
  role: "user" | "assistant" | "tool" | "system"
  text: string
  committed: boolean
}

export interface BrowserSnapshot {
  session: SessionSummary
  messages: BrowserMessage[]
  confirmations: Array<{ confirmation_id: string; summary: string }>
  presence: {
    state: string
    updated_at: string
    safe_error?: string
  }
  terminal?: {
    status: "completed" | "failed" | "cancelled" | "exhausted"
    detail?: string
    artifacts_persisted: boolean
  }
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
