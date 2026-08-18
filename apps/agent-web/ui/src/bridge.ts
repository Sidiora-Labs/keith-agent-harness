import type { BrowserProjectionBridge } from "./types"

interface KeithWasmModule {
  default(options: { module_or_path: string }): Promise<unknown>
  BrowserProjection: new () => BrowserProjectionBridge
}

export async function createBrowserProjection(): Promise<BrowserProjectionBridge> {
  const moduleUrl = "/assets/agent_web.js"
  const wasm = (await import(/* @vite-ignore */ moduleUrl)) as KeithWasmModule
  await wasm.default({ module_or_path: "/assets/agent_web_bg.wasm" })
  return new wasm.BrowserProjection()
}
