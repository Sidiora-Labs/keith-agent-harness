import fs from "node:fs"
import path from "node:path"

const root = path.resolve(import.meta.dirname, "..")
const styles = fs.readFileSync(path.join(root, "src/styles.css"), "utf8")
if (!styles.includes('@import "@openai/apps-sdk-ui/css";')) {
  throw new Error("Keith styles must import @openai/apps-sdk-ui/css")
}
const forbidden = [
  [/(?:#(?:[0-9a-f]{3,8})\b|\brgba?\(|\bhsla?\(|\boklch\()/i, "raw color"],
  [/(?:^|[;{]\s*)border(?:-color|-style|-width)?\s*:/im, "decorative border"],
  [/\b(?:glow|gradient|purple)\b/i, "forbidden treatment"]
]
for (const [pattern, label] of forbidden) {
  if (pattern.test(styles)) throw new Error(`Keith styles contain ${label}`)
}
for (const entry of fs.readdirSync(path.join(root, "src"), { recursive: true })) {
  if (typeof entry !== "string" || !entry.endsWith(".tsx")) continue
  const source = fs.readFileSync(path.join(root, "src", entry), "utf8")
  if (/\bstyle\s*=/.test(source)) throw new Error(`${entry} contains an inline style`)
}
