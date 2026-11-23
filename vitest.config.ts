import { configDefaults, defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";

export default defineConfig({
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  // local agent worktrees hold a second copy of every test
  test: { exclude: [...configDefaults.exclude, ".claude/worktrees/**"] },
});
