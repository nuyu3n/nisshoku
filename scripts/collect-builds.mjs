// Gathers the installers of every platform into one folder:
//
//   builds/<version>/
//     Nisshoku_<version>_windows_x64-setup.exe   (NSIS installer)
//     Nisshoku_<version>_windows_x64.msi
//     Nisshoku_<version>_android.apk             (signed release, all ABIs)
//     Nisshoku_<version>_android.aab             (if built, for Google Play)
//     Nisshoku_<version>_linux_amd64.AppImage / .deb, ..._linux_x86_64.rpm
//
//   node scripts/collect-builds.mjs
//
// It looks where each build leaves its files: target/release/bundle (Windows),
// gen/android/.../outputs (Android) and the WSL copy of the repository
// (~/nisshoku-build, see README) for Linux. Only files of the current product
// name and version count, newest first; a platform that was not built is
// reported and skipped. Update signatures (.sig) stay where they are: the
// release workflow publishes those.

import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const conf = JSON.parse(readFileSync(join(root, "apps/desktop/src-tauri/tauri.conf.json"), "utf8"));
const product = conf.productName;
const version = conf.version;
const out = join(root, "builds", version);

/** The newest file in `dir` whose name matches `re`, or null. */
function newest(dir, re) {
  if (!existsSync(dir)) return null;
  const hits = readdirSync(dir)
    .filter((n) => re.test(n))
    .map((n) => join(dir, n))
    .filter((p) => statSync(p).isFile())
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  return hits[0] ?? null;
}

const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const P = esc(product);
const V = esc(version);

const bundle = join(root, "target/release/bundle");
const android = join(root, "apps/desktop/src-tauri/gen/android/app/build/outputs");

/** The Linux bundle folder inside WSL, as a Windows path (\\wsl.localhost\...). */
function wslBundle() {
  if (process.platform !== "win32") return join(root, "target/release/bundle");
  const r = spawnSync("wsl", ["-d", "Debian", "--", "bash", "-lc", "wslpath -w ~/nisshoku-build/target/release/bundle"], {
    encoding: "utf8",
    env: { ...process.env, MSYS_NO_PATHCONV: "1" },
  });
  const p = r.status === 0 ? r.stdout.trim() : "";
  return p || null;
}
const linux = wslBundle();

const wanted = [
  ["Windows (NSIS)", join(bundle, "nsis"), new RegExp(`^${P}_${V}_x64-setup\\.exe$`), `${product}_${version}_windows_x64-setup.exe`],
  ["Windows (MSI)", join(bundle, "msi"), new RegExp(`^${P}_${V}_x64_ja-JP\\.msi$`), `${product}_${version}_windows_x64.msi`],
  ["Android (APK)", join(android, "apk/universal/release"), /^app-universal-release\.apk$/, `${product}_${version}_android.apk`],
  ["Android (AAB)", join(android, "bundle/universalRelease"), /^app-universal-release\.aab$/, `${product}_${version}_android.aab`],
  ["Linux (AppImage)", linux && join(linux, "appimage"), new RegExp(`^${P}_${V}_amd64\\.AppImage$`), `${product}_${version}_linux_amd64.AppImage`],
  ["Linux (deb)", linux && join(linux, "deb"), new RegExp(`^${P}_${V}_amd64\\.deb$`), `${product}_${version}_linux_amd64.deb`],
  ["Linux (rpm)", linux && join(linux, "rpm"), new RegExp(`^${P}-${V}-1\\.x86_64\\.rpm$`), `${product}_${version}_linux_x86_64.rpm`],
];

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
let n = 0;
for (const [label, dir, re, name] of wanted) {
  const file = dir && newest(dir, re);
  if (!file) {
    console.log(`  -  ${label}: not built`);
    continue;
  }
  copyFileSync(file, join(out, name));
  const when = new Date(statSync(file).mtimeMs).toLocaleString();
  const mb = (statSync(file).size / 1048576).toFixed(1);
  console.log(`  ✓  ${label}: ${name}  (${mb} MB, built ${when})`);
  n++;
}
console.log(`\n${n} file(s) in ${out}`);
