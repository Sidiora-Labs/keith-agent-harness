import fs from "node:fs"
import path from "node:path"

const root = path.resolve(import.meta.dirname, "..")
const output = path.resolve(root, "../static/ui")
const manifestPath = path.join(output, ".vite/manifest.json")
if (!fs.existsSync(manifestPath)) throw new Error("production web manifest is missing")
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"))
const entry = manifest["src/index.tsx"]
if (!entry?.isEntry || typeof entry.file !== "string") {
  throw new Error("production web entry is missing from the manifest")
}
const assets = [entry.file, ...(entry.css ?? [])]
for (const asset of assets) {
  const file = path.join(output, asset)
  if (!fs.existsSync(file) || fs.statSync(file).size === 0) {
    throw new Error(`production web asset is missing: ${asset}`)
  }
}
const bundle = assets
  .map((asset) => fs.readFileSync(path.join(output, asset), "utf8"))
  .join("\n")
for (const secretMarker of ["KEITH_WEB_LOGIN_SECRET", "authorization: bearer", "localStorage.setItem", "sessionStorage.setItem"]) {
  if (bundle.toLowerCase().includes(secretMarker.toLowerCase())) {
    throw new Error(`production bundle contains forbidden secret marker: ${secretMarker}`)
  }
}
