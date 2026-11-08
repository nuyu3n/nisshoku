// Builds the installers on this machine, signed for the updater with the key
// in ~/.tauri (made by `pnpm tauri signer generate`, kept outside the
// repository). Arguments go to `tauri build` (e.g. `--bundles nsis`).
//
//   node scripts/build-app.mjs
//
// Without the key, the installers are built without update signatures.

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const keyDir = join(homedir(), ".tauri");
const key = join(keyDir, "nisshoku-updater.key");
const password = join(keyDir, "nisshoku-updater.password");

const env = { ...process.env };
const args = ["tauri", "build", ...process.argv.slice(2)];
if (!env.TAURI_SIGNING_PRIVATE_KEY && existsSync(key)) {
  env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(key, "utf8").trim();
  env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = existsSync(password) ? readFileSync(password, "utf8").trim() : "";
} else if (!env.TAURI_SIGNING_PRIVATE_KEY) {
  console.warn(`No updater key at ${key}: building without update signatures.`);
  args.push("--config", JSON.stringify({ bundle: { createUpdaterArtifacts: false } }));
}

const result = spawnSync("pnpm", args, { cwd: join(root, "apps", "desktop"), env, stdio: "inherit", shell: process.platform === "win32" });
// Then gather every platform's installers into builds/<version>/.
if (result.status === 0) spawnSync(process.execPath, [join(root, "scripts", "collect-builds.mjs")], { stdio: "inherit" });
process.exit(result.status ?? 1);
