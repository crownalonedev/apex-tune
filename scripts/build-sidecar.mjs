// Builds the apex-timer tray helper and places it where Tauri's bundler
// expects sidecars: src-tauri/binaries/apex-timer-<target triple>[.exe].
// Run automatically before `tauri dev` and `tauri build`.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri");
const release = process.argv.includes("--release");
const triple =
  process.env.TAURI_ENV_TARGET_TRIPLE ||
  execFileSync("rustc", ["-vV"], { encoding: "utf8" })
    .split("\n")
    .find((l) => l.startsWith("host:"))
    .slice(5)
    .trim();

const args = ["build", "-p", "apex-timer", "--target", triple];
if (release) args.push("--release");
execFileSync("cargo", triple.includes("windows") && process.platform !== "win32" ? ["xwin", ...args] : args, { cwd: root, stdio: "inherit" });

const ext = triple.includes("windows") ? ".exe" : "";
const built = join(root, "target", triple, release ? "release" : "debug", `apex-timer${ext}`);
mkdirSync(join(root, "binaries"), { recursive: true });
copyFileSync(built, join(root, "binaries", `apex-timer-${triple}${ext}`));
console.log(`apex-timer sidecar ready for ${triple}`);
