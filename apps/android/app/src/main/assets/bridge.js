// Stands in for Tauri in the web view of the native app: the web editor
// (apps/desktop, `?embed=editor`) calls `window.__TAURI_INTERNALS__.invoke`
// as it does in the desktop window; here the calls go to the engine through
// the app (`NisshokuBridge`, ui/Editor.kt), and the engine's events come back.
(() => {
  if (window.__TAURI_INTERNALS__) return;
  const native = window.NisshokuBridge;
  const PROFILE = "__PROFILE__";
  const ORIGIN = "https://appassets.androidplatform.net";

  let seq = 0;
  const pending = new Map();
  /** event name → Map(event id → callback id) */
  const listeners = new Map();
  let nextEventId = 1;
  /** "plugin|event" → Map(channel id → next message index) */
  const pluginListeners = new Map();

  function transformCallback(callback, once) {
    const id = window.crypto.getRandomValues(new Uint32Array(1))[0];
    const prop = `_${id}`;
    Object.defineProperty(window, prop, {
      value: (result) => {
        if (once) Reflect.deleteProperty(window, prop);
        return callback && callback(result);
      },
      writable: false,
      configurable: true,
    });
    return id;
  }

  function unregisterCallback(id) {
    Reflect.deleteProperty(window, `_${id}`);
  }

  function emitLocal(event, payload) {
    const m = listeners.get(event);
    if (!m) return;
    for (const [eventId, handler] of [...m]) {
      const f = window[`_${handler}`];
      if (f) f({ event, id: eventId, payload });
    }
  }

  /** A Channel's id: the object itself (not serialized here), or its `__CHANNEL__:id`. */
  function channelId(channel) {
    if (channel && typeof channel === "object" && "id" in channel) return Number(channel.id);
    return Number(String(channel).replace("__CHANNEL__:", ""));
  }

  function toChannel(channel, message) {
    const id = channelId(channel);
    for (const m of pluginListeners.values()) {
      if (!m.has(id)) continue;
      const index = m.get(id);
      m.set(id, index + 1);
      const f = window[`_${id}`];
      if (f) f({ index, message });
    }
  }

  /** A call to the app, answered by `__nisshokuReply`. */
  function ask(start) {
    return new Promise((resolve, reject) => {
      const id = ++seq;
      pending.set(id, { resolve, reject });
      try {
        start(id);
      } catch (e) {
        pending.delete(id);
        reject(String(e));
      }
    });
  }

  function base64(bytes) {
    let s = "";
    const chunk = 0x8000;
    for (let i = 0; i < bytes.length; i += chunk) s += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk));
    return btoa(s);
  }

  function header(options, name) {
    const h = options && options.headers;
    if (!h) return "";
    if (typeof h.get === "function") return h.get(name) || "";
    return h[name] || "";
  }

  /** What Tauri's plugins did on the desktop, done by the app (or nothing). */
  function plugin(cmd, args) {
    const a = args || {};
    switch (cmd) {
      case "plugin:event|listen": {
        const id = nextEventId++;
        if (!listeners.has(a.event)) listeners.set(a.event, new Map());
        listeners.get(a.event).set(id, a.handler);
        return Promise.resolve(id);
      }
      case "plugin:event|unlisten":
        listeners.get(a.event)?.delete(a.eventId);
        return Promise.resolve(null);
      case "plugin:event|emit":
      case "plugin:event|emit_to":
        emitLocal(a.event, a.payload);
        return Promise.resolve(null);
      case "plugin:app|register_listener": {
        const key = `app|${a.event}`;
        if (!pluginListeners.has(key)) pluginListeners.set(key, new Map());
        pluginListeners.get(key).set(channelId(a.handler), 0);
        return Promise.resolve(null);
      }
      case "plugin:app|remove_listener":
        pluginListeners.get(`app|${a.event}`)?.delete(Number(a.channelId));
        return Promise.resolve(null);
      case "plugin:app|version":
        return Promise.resolve(native.appVersion());
      case "plugin:app|name":
        return Promise.resolve("日色");
      case "plugin:app|tauri_version":
        return Promise.resolve("2");
      case "plugin:opener|open_url":
        native.openUrl(a.url || a.path || "");
        return Promise.resolve(null);
      case "plugin:clipboard-manager|write_text":
        native.writeClipboard(a.text ?? a.data?.plainText?.text ?? "");
        return Promise.resolve(null);
      case "plugin:clipboard-manager|read_text":
        return Promise.resolve(native.readClipboard());
      case "plugin:dialog|open": {
        const o = a.options || {};
        const exts = (o.filters || []).flatMap((f) => f.extensions || []);
        return ask((id) => native.pickFile(id, JSON.stringify(exts), !!o.directory));
      }
      case "plugin:dialog|save": {
        const o = a.options || {};
        return ask((id) => native.saveFile(id, o.defaultPath || "note.txt"));
      }
      case "plugin:dialog|message":
        return Promise.resolve(null);
      case "plugin:dialog|ask":
      case "plugin:dialog|confirm":
        return Promise.resolve(window.confirm(a.message || ""));
      case "plugin:notification|is_permission_granted":
        return Promise.resolve(true);
      case "plugin:notification|request_permission":
        return Promise.resolve("granted");
      case "plugin:notification|notify":
        return Promise.resolve(null);
      default:
        // The window, the webview, the process, the updater: nothing to do here.
        if (/^plugin:(window|webview|process|updater|resources)\|/.test(cmd)) return Promise.resolve(null);
        return Promise.reject(`${cmd} is not available here`);
    }
  }

  function invoke(cmd, args, options) {
    if (cmd.startsWith("plugin:")) return plugin(cmd, args);
    if (args instanceof ArrayBuffer || ArrayBuffer.isView(args)) {
      const bytes = args instanceof ArrayBuffer ? new Uint8Array(args) : new Uint8Array(args.buffer, args.byteOffset, args.byteLength);
      return ask((id) => native.invokeRaw(id, base64(bytes), header(options, "x-name"), header(options, "x-mime")));
    }
    return ask((id) => native.invoke(id, cmd, JSON.stringify(args ?? {})));
  }

  function convertFileSrc(path) {
    const p = String(path).replace(/\\/g, "/");
    const rel = p.startsWith(PROFILE) ? p.slice(PROFILE.length) : p;
    return `${ORIGIN}/profile${rel.startsWith("/") ? "" : "/"}${rel.split("/").map(encodeURIComponent).join("/")}`;
  }

  window.__nisshokuReply = (id, ok, json) => {
    const p = pending.get(id);
    if (!p) return;
    pending.delete(id);
    let value = null;
    try {
      value = json === "" || json == null ? null : JSON.parse(json);
    } catch {
      value = json;
    }
    if (ok) p.resolve(value);
    else p.reject(value);
  };

  /** An event of the engine. */
  window.__nisshokuEmit = (event, payload) => emitLocal(event, payload);

  /** The system's back: true when the page took it. */
  window.__nisshokuBack = () => {
    const m = pluginListeners.get("app|back-button");
    if (!m || m.size === 0) return false;
    for (const id of [...m.keys()]) toChannel(`__CHANNEL__:${id}`, { canGoBack: false });
    return true;
  };

  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener(event, eventId) {
      listeners.get(event)?.delete(eventId);
    },
  };

  window.__TAURI_INTERNALS__ = {
    invoke,
    transformCallback,
    unregisterCallback,
    convertFileSrc,
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { windowLabel: "main", label: "main" },
    },
    plugins: {},
  };
  window.isTauri = true;
})();
