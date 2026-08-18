/* tslint:disable */
/* eslint-disable */

/**
 * The browser's only protocol authority. Solid renders the serialized projection and sends the
 * typed envelopes built here; it never reduces daemon events or constructs wire commands.
 */
export class BrowserProjection {
    free(): void;
    [Symbol.dispose](): void;
    apply_wire_message(encoded: string): string;
    attach_session(session_id: string): string;
    branch(session_id: string): string;
    cancel(session_id: string): string;
    create_child(session_id: string, objective: string): string;
    create_goal(session_id: string, objective: string): string;
    create_schedule(profile_id: string, session_id: string | null | undefined, prompt: string, interval_seconds: bigint): string;
    create_session(profile_id: string, workspace_id: string, title?: string | null): string;
    current_view(): string;
    export_session(session_id: string): string;
    list_sessions(profile_id?: string | null): string;
    constructor();
    query_memory(profile_id: string, query: string): string;
    resolve_confirmation(session_id: string, confirmation_id: string, allow: boolean): string;
    resume(session_id: string): string;
    retry(session_id: string): string;
    select_model(session_id: string, provider: string, model: string): string;
    set_background(profile_id: string, mode: string): string;
    steer(session_id: string, text: string): string;
    submit_prompt(session_id: string, text: string): string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_browserprojection_free: (a: number, b: number) => void;
    readonly browserprojection_apply_wire_message: (a: number, b: number, c: number, d: number) => void;
    readonly browserprojection_attach_session: (a: number, b: number, c: number, d: number) => void;
    readonly browserprojection_branch: (a: number, b: number, c: number, d: number) => void;
    readonly browserprojection_cancel: (a: number, b: number, c: number, d: number) => void;
    readonly browserprojection_create_child: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserprojection_create_goal: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserprojection_create_schedule: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: bigint) => void;
    readonly browserprojection_create_session: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly browserprojection_current_view: (a: number, b: number) => void;
    readonly browserprojection_export_session: (a: number, b: number, c: number, d: number) => void;
    readonly browserprojection_list_sessions: (a: number, b: number, c: number, d: number) => void;
    readonly browserprojection_new: () => number;
    readonly browserprojection_query_memory: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserprojection_resolve_confirmation: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly browserprojection_resume: (a: number, b: number, c: number, d: number) => void;
    readonly browserprojection_retry: (a: number, b: number, c: number, d: number) => void;
    readonly browserprojection_select_model: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly browserprojection_set_background: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserprojection_steer: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly browserprojection_submit_prompt: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly __wbindgen_export: (a: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export2: (a: number, b: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
