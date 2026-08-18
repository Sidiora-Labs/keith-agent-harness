import { defineConfig } from "vite"
import solid from "vite-plugin-solid"
import tailwindcss from "@tailwindcss/vite"

export default defineConfig({
  plugins: [tailwindcss(), solid()],
  build: {
    target: "es2022",
    outDir: "../static/ui",
    emptyOutDir: true,
    manifest: true,
    rollupOptions: {
      input: "src/index.tsx",
      output: {
        entryFileNames: "assets/keith-[hash].js",
        chunkFileNames: "assets/keith-[hash].js",
        assetFileNames: "assets/keith-[hash][extname]"
      }
    }
  }
})
