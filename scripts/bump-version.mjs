// Sets the version everywhere it is written: the Tauri config (which the
// installers, the updater and Android's versionName/versionCode follow), the
// web app's package.json and the Rust workspace.
//
//   node scripts/bump-version.mjs 0.2.0
//
// Then commit, and push a tag `v0.2.0`: the release workflow builds and
// publishes that version.

import { readFileSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const version = process.argv[2]?.replace(/^v/, "");
if (!version || !/^\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$/.test(version)) {
  console.error("usage: node scripts/bump-version.mjs <major.minor.patch[-pre]>");
  process.exit(1);
}
const root = join(dirname(fileURLToPath(import.meta.url)), "..");

function edit(path, change) {
  const file = join(root, path);
  const before = readFileSync(file, "utf8");
  const after = change(before);
  if (after === before) throw new Error(`no version found in ${path}`);
  writeFileSync(file, after);
  console.log(`  ${path}`);
}

console.log(`Version ${version}:`);
edit("apps/desktop/src-tauri/tauri.conf.json", (s) => s.replace(/("version":\s*")[^"]+(")/, `$1${version}$2`));
edit("apps/desktop/package.json", (s) => s.replace(/("version":\s*")[^"]+(")/, `$1${version}$2`));
edit("Cargo.toml", (s) => s.replace(/(\[workspace\.package\][^[]*?version\s*=\s*")[^"]+(")/, `$1${version}$2`));
console.log(`\nNext: update Cargo.lock (cargo check), commit, then\n  git tag v${version} && git push origin v${version}`);
