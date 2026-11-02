"use strict";

const TOKEN_KEY = "terax-remote-token";
const PREFS_KEY = "terax-remote-prefs";
const SCREEN_INTERVAL_MS = 700;
const LIST_INTERVAL_MS = 3000;
const SCREEN_LINES = 400;

const $ = (id) => document.getElementById(id);
const els = {
  tabs: $("tabs"),
  space: $("space"),
  screen: $("screen"),
  wrap: $("screen-wrap"),
  empty: $("empty"),
  toBottom: $("to-bottom"),
  input: $("input"),
  form: $("send-form"),
  toast: $("toast"),
  linkDot: $("link-dot"),
  wrapToggle: $("wrap"),
};

function storageGet(store, key) {
  try {
    return store.getItem(key);
  } catch {
    return null;
  }
}

function storageSet(store, key, value) {
  try {
    store.setItem(key, value);
  } catch {}
}

function takeToken() {
  const match = /(?:^|[#&])k=([0-9a-f]{64})(?:&|$)/.exec(location.hash);
  if (match) {
    storageSet(sessionStorage, TOKEN_KEY, match[1]);
    history.replaceState(null, "", location.pathname);
    return match[1];
  }
  return storageGet(sessionStorage, TOKEN_KEY);
}

const token = takeToken();
function loadPrefs() {
  const defaults = { fontSize: 0, wrap: false };
  try {
    const saved = JSON.parse(storageGet(localStorage, PREFS_KEY) || "{}");
    return {
      fontSize: Number.isFinite(saved.fontSize) ? saved.fontSize : 0,
      wrap: saved.wrap === true,
    };
  } catch {
    return defaults;
  }
}

const prefs = loadPrefs();

const state = {
  terminals: [],
  leaf: null,
  spaceId: null,
  hash: null,
  cols: 80,
  stopped: false,
  stickToBottom: true,
};

class ApiError extends Error {
  constructor(status, message) {
    super(message);
    this.status = status;
  }
}

async function api(method, params) {
  const response = await fetch(`/api/${method}`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(params ?? {}),
    cache: "no-store",
  });
  const body = await response.json().catch(() => ({}));
  if (!response.ok) throw new ApiError(response.status, body.error || response.statusText);
  return body;
}

let toastTimer = 0;
function toast(message, bad) {
  els.toast.textContent = message;
  els.toast.classList.toggle("bad", Boolean(bad));
  els.toast.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    els.toast.hidden = true;
  }, 2200);
}

function setLink(ok) {
  els.linkDot.className = `dot ${ok ? "ok" : "bad"}`;
}

function showEmpty(message) {
  els.empty.textContent = message;
  els.empty.hidden = false;
  els.screen.hidden = true;
}

function stop(message) {
  state.stopped = true;
  setLink(false);
  showEmpty(message);
  els.input.disabled = true;
  for (const button of document.querySelectorAll("footer button")) button.disabled = true;
}

function handleError(error) {
  if (error instanceof ApiError && error.status === 401) {
    stop("This link has expired. Turn phone access on again in Terax and scan the new QR code.");
    return;
  }
  setLink(false);
}

function preferredIn(terminals) {
  return (
    terminals.find((terminal) => terminal.agentStatus === "waiting") ||
    terminals.find((terminal) => terminal.active) ||
    terminals[0] ||
    null
  );
}

function renderSpaces() {
  const spaces = new Map();
  for (const terminal of state.terminals) {
    const entry = spaces.get(terminal.spaceId) || { name: terminal.space, count: 0, waiting: false };
    entry.count += 1;
    entry.waiting ||= terminal.agentStatus === "waiting";
    spaces.set(terminal.spaceId, entry);
  }
  els.space.replaceChildren(
    ...[...spaces].map(([id, entry]) => {
      const option = document.createElement("option");
      option.value = id;
      option.textContent = `${entry.waiting ? "\u25cf " : ""}${entry.name} (${entry.count})`;
      return option;
    }),
  );
  els.space.hidden = spaces.size === 0;
  if (state.spaceId !== null) els.space.value = state.spaceId;
}

function renderTabs() {
  renderSpaces();
  els.tabs.replaceChildren(
    ...state.terminals
      .filter((terminal) => terminal.spaceId === state.spaceId)
      .map((terminal) => {
        const button = document.createElement("button");
        button.type = "button";
        button.className = terminal.cold ? "tab cold" : "tab";
        button.setAttribute("role", "tab");
        button.setAttribute("aria-selected", String(terminal.leaf === state.leaf));
        const dot = document.createElement("span");
        dot.className = `dot ${terminal.agentStatus || ""}`;
        const label = document.createElement("span");
        label.className = "label";
        label.textContent = terminal.agent ? `${terminal.agent} · ${terminal.title}` : terminal.title;
        button.append(dot, label);
        button.addEventListener("click", () => selectLeaf(terminal.leaf));
        return button;
      }),
  );
  els.tabs.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest", inline: "nearest" });
}

function selectLeaf(leaf) {
  if (state.leaf === leaf) return;
  const terminal = state.terminals.find((item) => item.leaf === leaf);
  state.leaf = leaf;
  state.spaceId = terminal ? terminal.spaceId : state.spaceId;
  state.hash = null;
  state.stickToBottom = true;
  els.screen.textContent = "";
  renderTabs();
  void refreshScreen();
}

els.space.addEventListener("change", () => {
  state.spaceId = els.space.value;
  const target = preferredIn(state.terminals.filter((terminal) => terminal.spaceId === state.spaceId));
  if (target) selectLeaf(target.leaf);
  else renderTabs();
});

async function refreshList() {
  try {
    const result = await api("list");
    setLink(true);
    state.terminals = result.terminals || [];
    const current = state.terminals.find((terminal) => terminal.leaf === state.leaf);
    if (current) {
      state.spaceId = current.spaceId;
    } else {
      const preferred = preferredIn(state.terminals);
      state.leaf = preferred ? preferred.leaf : null;
      state.spaceId = preferred ? preferred.spaceId : null;
      state.hash = null;
    }
    renderTabs();
    if (state.leaf === null) showEmpty("No terminals are open in Terax.");
  } catch (error) {
    handleError(error);
  }
}

function applyFont() {
  const width = els.wrap.clientWidth - 20;
  const fitted = Math.max(6, Math.min(14, width / (state.cols * 0.61)));
  const size = prefs.fontSize > 0 ? prefs.fontSize : fitted;
  els.screen.style.fontSize = `${size.toFixed(2)}px`;
  els.screen.classList.toggle("wrap", prefs.wrap);
  els.wrapToggle.setAttribute("aria-pressed", String(prefs.wrap));
}

function nearBottom() {
  const el = els.wrap;
  return el.scrollHeight - el.scrollTop - el.clientHeight < 48;
}

async function refreshScreen() {
  if (state.stopped || state.leaf === null || document.hidden) return;
  const leaf = state.leaf;
  try {
    const result = await api("screen", { leaf, lines: SCREEN_LINES, since: state.hash });
    setLink(true);
    if (leaf !== state.leaf || result.unchanged) return;
    if (result.starting) {
      state.hash = null;
      showEmpty("Starting terminal");
      return;
    }
    state.hash = result.hash;
    if (result.cols && result.cols !== state.cols) {
      state.cols = result.cols;
      applyFont();
    }
    els.empty.hidden = true;
    els.screen.hidden = false;
    els.screen.textContent = result.text;
    if (state.stickToBottom) els.wrap.scrollTop = els.wrap.scrollHeight;
  } catch (error) {
    handleError(error);
  }
}

async function send(method, params) {
  if (state.leaf === null) return false;
  try {
    await api(method, { leaf: state.leaf, ...params });
    state.stickToBottom = true;
    setTimeout(refreshScreen, 120);
    return true;
  } catch (error) {
    handleError(error);
    toast(error.message || "Could not send", true);
    return false;
  }
}

function savePrefs() {
  storageSet(localStorage, PREFS_KEY, JSON.stringify(prefs));
  applyFont();
}

function autosize() {
  els.input.style.height = "auto";
  els.input.style.height = `${Math.min(els.input.scrollHeight, 120)}px`;
}

els.form.addEventListener("submit", async (event) => {
  event.preventDefault();
  const text = els.input.value;
  if (await send("send", { text, enter: true })) {
    els.input.value = "";
    autosize();
  }
});

els.input.addEventListener("input", autosize);
els.input.addEventListener("keydown", (event) => {
  if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
    event.preventDefault();
    els.form.requestSubmit();
  }
});

for (const button of document.querySelectorAll("[data-key]")) {
  button.addEventListener("click", () => void send("key", { key: button.dataset.key }));
}

$("font-down").addEventListener("click", () => {
  const current = parseFloat(els.screen.style.fontSize) || 11;
  prefs.fontSize = Math.max(5, current - 1);
  savePrefs();
});
$("font-up").addEventListener("click", () => {
  const current = parseFloat(els.screen.style.fontSize) || 11;
  prefs.fontSize = Math.min(22, current + 1);
  savePrefs();
});
els.wrapToggle.addEventListener("click", () => {
  prefs.wrap = !prefs.wrap;
  savePrefs();
});

els.wrap.addEventListener("scroll", () => {
  state.stickToBottom = nearBottom();
  els.toBottom.hidden = state.stickToBottom;
});
els.toBottom.addEventListener("click", () => {
  state.stickToBottom = true;
  els.wrap.scrollTop = els.wrap.scrollHeight;
});

window.addEventListener("resize", applyFont);
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) {
    void refreshList();
    void refreshScreen();
  }
});

function loop(fn, interval) {
  const tick = async () => {
    if (state.stopped) return;
    if (!document.hidden) await fn();
    setTimeout(tick, interval);
  };
  void tick();
}

if (!token) {
  stop("Open this page from the QR code or link shown in Terax.");
} else {
  applyFont();
  void refreshList().then(() => {
    loop(refreshScreen, SCREEN_INTERVAL_MS);
    setTimeout(() => loop(refreshList, LIST_INTERVAL_MS), LIST_INTERVAL_MS);
  });
}
