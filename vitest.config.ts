import { defineConfig, mergeConfig } from "vitest/config"
import viteConfig from "./vite.config.ts"

// The app's Vite config (plugins, React Compiler, the "@" alias) plus the test settings.
export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: "jsdom",
      include: ["src/**/*.test.{ts,tsx}"],
      setupFiles: ["src/test/setup.ts"],
      // Tests import describe/it/expect from "vitest" explicitly; no globals.
      globals: false,
      css: false,
    },
  }),
)
