// Пересобирает src-tauri и один раз запускает бинарник в режиме
// "только экспорт биндингов" (см. DREAMLAUNCHER_EXPORT_BINDINGS_ONLY в
// src-tauri/src/lib.rs) — окно не открывается, процесс сразу завершается.
// Нужен свой скрипт, а не голый `cargo run`, потому что путь экспорта в
// tauri-specta ("../src/ipc/bindings.ts") относительный и должен
// резолвиться от src-tauri/, а не от корня workspace.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const srcTauriDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "src-tauri");

const build = spawnSync("cargo", ["build", "-p", "dreamlauncher"], { cwd: srcTauriDir, stdio: "inherit" });
if (build.status !== 0) process.exit(build.status ?? 1);

const exe = process.platform === "win32" ? "../target/debug/dreamlauncher.exe" : "../target/debug/dreamlauncher";
const run = spawnSync(exe, [], {
  cwd: srcTauriDir,
  stdio: "inherit",
  env: { ...process.env, DREAMLAUNCHER_EXPORT_BINDINGS_ONLY: "1" },
});
process.exit(run.status ?? 1);
