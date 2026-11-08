// Attachments stored as Joplin resources: images are written as
// `![](:/<id>)`, other files as links `[name](:/<id>)`; both are shown from
// the resource file on this device.

import { Image } from "@tiptap/extension-image";
import type { Editor } from "@tiptap/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, isTauri, type ResourceFile } from "../api";
import { report, useStore } from "../store";
import { t } from "../i18n";

const files = new Map<string, ResourceFile>();
let loaded: Promise<void> | null = null;

/** Learns where the resource files are (again after syncs bring new ones). */
export function loadResources(): Promise<void> {
  loaded = api.resourceFiles().then(
    (list) => {
      files.clear();
      for (const r of list) files.set(r.id, r);
    },
    () => {},
  );
  return loaded;
}

/** Points the images on the page at their files again (after a sync brought files that were missing). */
export function refreshImages() {
  for (const img of document.querySelectorAll<HTMLImageElement>("img[data-src^=':/']")) {
    const url = resourceUrl(img.dataset.src);
    if (!url) continue;
    if (img.getAttribute("src") !== url || !img.complete || img.naturalWidth === 0) {
      img.removeAttribute("src");
      img.setAttribute("src", url);
    }
  }
}

export function resourcesReady() {
  return loaded ?? loadResources();
}

export function resource(id: string): ResourceFile | undefined {
  return files.get(id);
}

export function assetUrl(path: string): string {
  return isTauri ? convertFileSrc(path) : "";
}

export function resourceUrl(src: string | null | undefined): string {
  if (!src) return "";
  const m = /^:\/([0-9a-f]{32})/.exec(src);
  if (!m) return src;
  const r = files.get(m[1]);
  return r ? assetUrl(r.path) : "";
}

export const ResourceImage = Image.extend({
  renderHTML({ HTMLAttributes }) {
    const src = HTMLAttributes.src as string | undefined;
    return ["img", { ...HTMLAttributes, src: resourceUrl(src), "data-src": src, loading: "lazy" }];
  },
});

function iconFor(mime: string): string {
  if (mime === "application/pdf") return "📄";
  if (mime.startsWith("audio/")) return "🎵";
  if (mime.startsWith("video/")) return "🎞";
  if (mime.startsWith("text/")) return "📝";
  return "📎";
}

function insertResource(editor: Editor, r: ResourceFile, name: string, pos?: number) {
  files.set(r.id, r);
  const chain = pos !== undefined ? editor.chain().insertContentAt(pos, "") : editor.chain().focus();
  if (r.mime.startsWith("image/")) {
    chain.insertContent({ type: "image", attrs: { src: `:/${r.id}`, alt: name.replace(/\.[^.]+$/, "") } }).run();
  } else {
    chain
      .insertContent([
        { type: "text", text: `${iconFor(r.mime)} ${name}`, marks: [{ type: "link", attrs: { href: `:/${r.id}` } }] },
        { type: "text", text: " " },
      ])
      .run();
  }
}

/** Saves pasted or dropped files and inserts them at `pos` (or the selection). */
export async function insertFiles(editor: Editor, list: FileList | File[], pos?: number) {
  const all = Array.from(list);
  for (const file of all) {
    try {
      const data = new Uint8Array(await file.arrayBuffer());
      const r = await api.saveResource(file.name || "file", file.type || "application/octet-stream", data);
      insertResource(editor, r, file.name || r.title, pos);
    } catch (e) {
      report(`${t("image.failed")}: ${e}`);
    }
  }
  return all.length > 0;
}

/** Asks for a file (an image, if `images`) and inserts it. */
export async function chooseFile(editor: Editor, images = false) {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const path = await open(images ? { filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"] }] } : {});
  if (typeof path !== "string") return;
  const r = await api.importResource(path).catch(report);
  if (!r) return;
  insertResource(editor, r, path.split(/[\\/]/).pop() ?? r.title);
}

export const chooseImage = (editor: Editor) => chooseFile(editor, true);

/** Shows a PDF inside the app, opens other attachments with their app. */
export async function openAttachment(id: string) {
  await resourcesReady();
  const r = files.get(id);
  if (r && r.mime === "application/pdf" && isTauri) {
    useStore.getState().set({ preview: { title: r.title, url: assetUrl(r.path), id } });
    return;
  }
  // Files that run as programs could have come from anyone who can write to
  // the sync target: ask first.
  const ext = (r?.path.match(/\.([A-Za-z0-9]+)$/)?.[1] ?? "").toLowerCase();
  if (RISKY.has(ext)) {
    const { dialogs } = await import("../components/Dialogs");
    const ok = await dialogs.confirm(t("attach.riskyTitle"), t("attach.riskyDetail", { name: r?.title ?? ext }), { okLabel: t("attach.riskyOk"), danger: true });
    if (!ok) return;
  }
  await api.openResource(id, RISKY.has(ext)).catch(report);
}

/** Extensions that run code when opened. */
const RISKY = new Set(["exe", "bat", "cmd", "com", "scr", "pif", "msi", "msp", "ps1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta", "lnk", "reg", "cpl", "jar", "sh", "app", "desktop", "appimage", "run", "apk", "dll", "url", "scf", "inf", "chm", "ps1xml", "psm1", "msc", "gadget", "application", "xbap"]);

/** Whether a `:/id` link points to an attachment (and not to a note). */
export function isAttachment(id: string): boolean {
  return files.has(id);
}
