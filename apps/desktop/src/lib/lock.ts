// Locking a note with a password: the body is encrypted on this device
// (PBKDF2-SHA256 and AES-GCM, WebCrypto) and stored as a block that Joplin
// shows as is. The title stays readable.

const MARK = "```nisshoku-lock";
const ITERATIONS = 310_000;

function b64(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
}

function unb64(s: string): Uint8Array {
  return Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
}

async function key(password: string, salt: Uint8Array): Promise<CryptoKey> {
  const base = await crypto.subtle.importKey("raw", new TextEncoder().encode(password), "PBKDF2", false, ["deriveKey"]);
  return crypto.subtle.deriveKey(
    { name: "PBKDF2", salt: salt as BufferSource, iterations: ITERATIONS, hash: "SHA-256" },
    base,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
}

const BLOCK = /```nisshoku-lock\s*\nv1:([A-Za-z0-9+/=]+):([A-Za-z0-9+/=]+):([A-Za-z0-9+/=]+)\s*\n```/;

/** Whether the body is a locked one (an actual encrypted block, not just the word). */
export function isLocked(body: string): boolean {
  return body.includes(MARK) && BLOCK.test(body);
}

export async function lockBody(text: string, password: string, notice: string): Promise<string> {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const k = await key(password, salt);
  const ct = new Uint8Array(await crypto.subtle.encrypt({ name: "AES-GCM", iv: iv as BufferSource }, k, new TextEncoder().encode(text)));
  return `${notice}\n\n${MARK}\nv1:${b64(salt)}:${b64(iv)}:${b64(ct)}\n\`\`\`\n`;
}

/** The text of a locked body; throws if the password is wrong. */
export async function unlockBody(body: string, password: string): Promise<string> {
  const m = BLOCK.exec(body);
  if (!m) throw new Error("not a locked note");
  const k = await key(password, unb64(m[1]));
  const plain = await crypto.subtle.decrypt({ name: "AES-GCM", iv: unb64(m[2]) as BufferSource }, k, unb64(m[3]) as BufferSource);
  return new TextDecoder().decode(plain);
}
