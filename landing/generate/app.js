import init, { list_chains, chain_details, validate_pattern, estimate_difficulty } from "./wasm/vanity_wasm.js";
import { POPULAR, DEFAULT_CHAIN, META, EXPLORERS, splitLabel } from "./chain-meta.js";

// No cookies, no network beyond this page's own files. localStorage holds only the theme and the
// notice version below, and sw.js caches only static files — never keys or addresses.

const $ = (id) => document.getElementById(id);
const BENCH_MS = 1000;
const WARMUP_MS = 400;
const REDUCED_MOTION = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const REEL_MS = REDUCED_MOTION ? 900 : 110;

const state = {
  module: null,
  chains: [],
  chain: null, // { id, name, hint, sample, fixed: bool[] }
  speeds: new Map(), // chain id + case mode -> measured keys/sec, this tab only
  estimate: null,
  pool: null,
  bench: null,
  startedAt: 0,
  result: null,
  saved: false,
  locked: [], // board positions held by the user's word
};

/* ---------------- connection status ---------------- */

function renderNet() {
  const offline = !navigator.onLine;
  $("net").classList.toggle("offline", offline);
  $("net").classList.toggle("online", !offline);
  $("netText").textContent = offline ? "Offline — nothing can leave" : "Online — works offline too";
  $("net").title = $("netText").textContent;
}
window.addEventListener("online", renderNet);
window.addEventListener("offline", renderNet);
renderNet();

if ("serviceWorker" in navigator) {
  navigator.serviceWorker.register("sw.js").catch(() => {
    // Offline support is a bonus; the page works the same without it.
  });
}

/* ---------------- local preferences ---------------- */

// Bump when the notice text changes so everyone sees the new version once.
const CONSENT_VERSION = "2";

function readPref(key) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writePref(key, value) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Private mode or storage disabled: the page still works, it just asks again next visit.
  }
}

/* ---------------- theme ---------------- */

function renderThemeButton() {
  const light = document.documentElement.dataset.theme === "light";
  $("themeToggle").setAttribute("aria-label", light ? "Switch to dark theme" : "Switch to light theme");
}

$("themeToggle").addEventListener("click", () => {
  const next = document.documentElement.dataset.theme === "light" ? "dark" : "light";
  document.documentElement.dataset.theme = next;
  writePref("vanity.theme", next);
  renderThemeButton();
});

renderThemeButton();

/* ---------------- consent ---------------- */

const consent = $("consent");
const pledges = [...consent.querySelectorAll(".pledge input")];
const accepted = () => readPref("vanity.consent") === CONSENT_VERSION;

pledges.forEach((box) =>
  box.addEventListener("change", () => {
    $("consentOk").disabled = !pledges.every((b) => b.checked);
  }),
);
consent.addEventListener("cancel", (e) => {
  if (!accepted()) e.preventDefault();
});
consent.addEventListener("close", () => {
  if (accepted()) $("shell").inert = false;
});
$("consentForm").addEventListener("submit", (e) => {
  e.preventDefault();
  if (!pledges.every((b) => b.checked)) return;
  writePref("vanity.consent", CONSENT_VERSION);
  consent.close();
  $("shell").inert = false;
  focusMode();
});

$("safetyLink").addEventListener("click", () => {
  const seen = accepted();
  pledges.forEach((b) => (b.checked = seen));
  $("consentOk").disabled = !seen;
  consent.showModal();
});

if (accepted()) {
  $("shell").inert = false;
} else {
  consent.showModal();
}

/* ---------------- modes: setup → running → done ---------------- */

let mode = "setup";

function setMode(next) {
  mode = next;
  $("shell").dataset.mode = next;
  $("setupPanel").hidden = next !== "setup";
  $("runPanel").hidden = next !== "running";
  $("donePanel").hidden = next !== "done";
  const editable = next === "setup";
  if (!editable) closePosMenu(false);
  for (const id of ["chainChip", "position", "word"]) $(id).disabled = !editable;
  if (boardTiles.length) fitBoard(boardTiles.length);
  updatePlate();
  focusMode();
}

function focusMode() {
  if (consent.open) return;
  const target = { setup: "word", running: "stop", done: "download" }[mode];
  $(target)?.focus({ preventScroll: true });
}

function announce(text) {
  $("announce").textContent = text;
}

/* ---------------- helpers ---------------- */

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function tickerOf(id) {
  return META[id]?.ticker || id.slice(0, 4).toUpperCase();
}

function logo(id, size) {
  const img = el("img", "logo");
  img.src = `icons/${tickerOf(id).toLowerCase()}.png`;
  img.alt = "";
  img.width = img.height = size;
  img.decoding = "async";
  return img;
}

/* ---------------- chain picker ---------------- */

const sheet = $("chainSheet");

function orderedChains() {
  const byId = new Map(state.chains.map((c) => [c.id, c]));
  const top = POPULAR.map((id) => byId.get(id)).filter(Boolean);
  const rest = state.chains
    .filter((c) => !POPULAR.includes(c.id))
    .sort((a, b) => a.name.localeCompare(b.name));
  return [...top, ...rest];
}

// Lower is better; null means no match. Exact ticker beats name, name beats nicknames ("base" → EVM, not "base32").
function matchScore(c, q) {
  const meta = META[c.id];
  const title = splitLabel(c.name).title.toLowerCase();
  const ticker = (meta?.ticker || "").toLowerCase();
  const words = (meta?.words || "").split(/\s+/);
  const hay = `${c.name} ${c.id} ${ticker} ${meta?.words || ""}`.toLowerCase();
  if (!q.split(/\s+/).every((w) => hay.includes(w))) return null;
  if (q === c.id || q === ticker) return 0;
  if (title.startsWith(q)) return 1;
  if (words.includes(q)) return 2;
  if (words.some((w) => w.startsWith(q))) return 3;
  return 4;
}

function chainTile(c) {
  const { title, detail } = splitLabel(c.name);
  const btn = el("button", "chain-tile");
  btn.type = "button";
  btn.dataset.id = c.id;
  if (state.chain?.id === c.id) {
    btn.classList.add("selected");
    btn.setAttribute("aria-current", "true");
  }
  btn.setAttribute("aria-label", `${title}${detail ? `, ${detail}` : ""}`);
  const img = logo(c.id, 28);
  img.addEventListener("error", () => img.replaceWith(el("span", "logo-text", tickerOf(c.id))), { once: true });
  const text = el("span", "tile-text");
  text.append(el("span", "tile-title", title));
  if (detail) text.append(el("span", "tile-detail", detail));
  btn.append(img, text);
  btn.addEventListener("click", () => {
    sheet.close();
    pickChain(c.id);
  });
  return btn;
}

function renderChains(filter = "") {
  const grid = $("chainGrid");
  const q = filter.trim().toLowerCase();
  const all = orderedChains();
  const list = q
    ? all
        .map((c, i) => ({ c, i, s: matchScore(c, q) }))
        .filter((x) => x.s !== null)
        .sort((a, b) => a.s - b.s || a.i - b.i)
        .map((x) => x.c)
    : all;
  grid.replaceChildren();
  if (!list.length) {
    grid.append(el("p", "empty", `No chain matches “${filter.trim()}”. Try a name or ticker, like SOL or ETH.`));
    return;
  }
  list.forEach((c) => grid.append(chainTile(c)));
}

function openSheet() {
  if (mode !== "setup") return;
  $("chainSearch").value = "";
  renderChains();
  sheet.showModal();
  if (window.matchMedia("(pointer: fine)").matches) $("chainSearch").focus();
  else sheet.querySelector(".chain-tile.selected")?.focus();
}

$("chainChip").addEventListener("click", openSheet);
$("chainSearch").addEventListener("input", (e) => renderChains(e.target.value));
$("chainSearch").addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    $("chainGrid").querySelector("button[data-id]")?.click();
  }
});
sheet.addEventListener("click", (e) => {
  if (e.target === sheet) sheet.close();
});
sheet.addEventListener("close", () => {
  if (mode === "setup") $("word").focus({ preventScroll: true });
});
document.addEventListener("keydown", (e) => {
  if (e.key !== "/" || mode !== "setup" || consent.open || sheet.open) return;
  // "/" never appears in an address, so it can open the picker even from the word field.
  const active = document.activeElement;
  if (active?.matches("input, textarea, select") && active.id !== "word") return;
  e.preventDefault();
  openSheet();
});

// Positions that look the same in several fresh addresses are fixed by the format (0x, bc1q, dashes…).
function fixedPositions(samples) {
  const first = samples[0];
  const sameLength = samples.every((s) => s.length === first.length);
  return [...first].map((ch, i) => (sameLength || i < 4 ? samples.every((s) => s[i] === ch) : false));
}

function pickChain(id) {
  const d = chain_details(id);
  const samples = [d.sample, ...[1, 2, 3].map(() => chain_details(id).sample)];
  state.chain = { id, name: d.name, hint: d.hint, sample: d.sample, fixed: fixedPositions(samples) };
  const { title } = splitLabel(d.name);
  $("chainName").textContent = title;
  $("chainLogo").src = `icons/${tickerOf(id).toLowerCase()}.png`;
  const { color, color2, mono } = META[id] ?? {};
  const [a, b] = mono
    ? ["var(--mono-a)", "var(--mono-b)"]
    : [color || "var(--violet)", color2 || (color ? `color-mix(in srgb, ${color} 50%, white)` : "var(--cyan)")];
  $("shell").style.setProperty("--chain-a", a);
  $("shell").style.setProperty("--chain-b", b);
  $("chainChip").setAttribute("aria-label", `Blockchain: ${d.name}. Change`);
  // Case only matters where addresses mix upper and lower case (base58, TON).
  const free = [...d.sample].filter((_, i) => !state.chain.fixed[i]).join("");
  $("exactWrap").hidden = !(/[A-Z]/.test(free) && /[a-z]/.test(free));
  if ($("exactWrap").hidden) $("exact").checked = false;
  state.estimate = null;
  renderPreview();
  checkPattern();
}

/* ---------------- the board ---------------- */

const board = $("board");
let boardTiles = [];
let boardCols = 1;

const WAVE_STEP = 2;
const WAVE_TAIL = 6;
let reelHead = 0;

// The rim beam crosses the plate once per tile-wave pass.
function syncSweep() {
  const sweepMs = ((boardCols + WAVE_TAIL) / WAVE_STEP) * REEL_MS;
  $("plate").style.setProperty("--sweep", `${Math.round(sweepMs)}ms`);
}

function leadLength() {
  const f = state.chain.fixed;
  let n = 0;
  while (n < f.length && f[n]) n++;
  return n;
}

function readPattern() {
  const word = $("word").value.trim();
  const at = position;
  return {
    chain: state.chain.id,
    prefix: at === "prefix" ? word : "",
    suffix: at === "suffix" ? word : "",
    contains: at === "contains" ? word : "",
    ignoreCase: !$("exact").checked,
  };
}

// Where the typed word would sit in a sample address — an illustration, not a promise.
function previewLayout() {
  const sample = state.chain.sample;
  const chars = [...sample];
  const locked = new Array(chars.length).fill(false);
  const word = $("word").value.trim().split(",")[0];
  if (!word) return { chars, locked };
  const lead = leadLength();
  const leadText = sample.slice(0, lead);
  const at = position;
  let start;
  if (at === "prefix") {
    start = lead && word.toLowerCase().startsWith(leadText.toLowerCase()) ? 0 : lead;
  } else if (at === "suffix") {
    start = chars.length - word.length;
  } else {
    start = Math.max(lead, Math.floor((chars.length - word.length) / 2));
  }
  start = Math.max(0, Math.min(start, chars.length - word.length));
  [...word].forEach((ch, i) => {
    const idx = start + i;
    if (idx < 0 || idx >= chars.length) return;
    chars[idx] = ch === "*" ? "?" : ch;
    locked[idx] = true;
  });
  return { chars, locked };
}

function fitBoard(count) {
  const avail = $("composer").clientWidth || $("shell").clientWidth;
  const narrow = avail < 600;
  const inset = narrow ? 22 : 30; // .plate horizontal padding + border
  const width = Math.min(1000, avail) - inset;
  const gap = 2;
  const wantTile = narrow ? (mode === "done" ? 16 : 21) : 20;
  const maxTile = narrow ? 26 : 28;
  let cols = count;
  for (let rows = 1; rows <= 8; rows++) {
    cols = Math.ceil(count / rows);
    if ((width - gap * (cols - 1)) / cols >= wantTile) break;
  }
  const tile = Math.min(maxTile, Math.floor((width - gap * (cols - 1)) / cols));
  boardCols = cols;
  syncSweep();
  board.style.setProperty("--cols", String(cols));
  board.style.setProperty("--tile", `${tile}px`);
  board.style.setProperty("--gap", `${gap}px`);
}

function buildBoard(count) {
  if (boardTiles.length !== count) {
    board.replaceChildren();
    boardTiles = Array.from({ length: count }, () => {
      const t = el("span", "tile");
      board.append(t);
      return t;
    });
  }
  fitBoard(count);
  updatePlate();
}

function updatePlate() {
  if (!state.chain) return;
  const label = {
    setup: `Example ${state.chain.name} address`,
    running: `Searching ${state.chain.name} addresses…`,
    done: `Your ${state.chain.name} address`,
  }[mode];
  $("plateLabel").textContent = label;
  $("plateCount").textContent = `${boardTiles.length} characters`;
}

function paintTiles(chars, locked, fixed) {
  buildBoard(chars.length);
  chars.forEach((ch, i) => {
    const t = boardTiles[i];
    if (t.textContent !== ch) {
      t.textContent = ch;
      if (!REDUCED_MOTION && mode !== "done") flip(t);
    }
    t.classList.toggle("lock", Boolean(locked[i]));
    t.classList.toggle("fixed", Boolean(fixed?.[i]) && !locked[i]);
  });
}

function flip(t) {
  t.classList.toggle("flap-b", t.classList.contains("flap-a"));
  t.classList.toggle("flap-a", !t.classList.contains("flap-b"));
}

function renderPreview() {
  const { chars, locked } = previewLayout();
  state.locked = locked;
  paintTiles(chars, locked, state.chain.fixed);
  board.setAttribute("aria-label", `Example ${state.chain.name} address: ${chars.join("")}`);
  $("word").size = Math.max(4, $("word").value.length + 1);
}

new ResizeObserver(() => {
  if (boardTiles.length) fitBoard(boardTiles.length);
}).observe($("composer"));

/* ---------------- pattern + estimate ---------------- */

const speedKey = (p) => `${p.chain}:${p.ignoreCase}`;

let checkTimer = 0;
let benchToken = 0;

function scheduleCheck() {
  clearTimeout(checkTimer);
  checkTimer = setTimeout(checkPattern, 220);
}

$("word").addEventListener("input", () => {
  renderPreview();
  scheduleCheck();
});
/* ---------------- position menu: starts / ends / contains ---------------- */

let position = "prefix";
const posButton = $("position");
const posMenu = $("positionMenu");
const posItems = [...posMenu.querySelectorAll(".menu-item")];

function setPosition(value) {
  position = value;
  const item = posItems.find((i) => i.dataset.value === value);
  $("positionLabel").textContent = item.textContent;
  posItems.forEach((i) => i.setAttribute("aria-selected", String(i === item)));
  renderPreview();
  checkPattern();
}

function openPosMenu() {
  posMenu.hidden = false;
  posButton.setAttribute("aria-expanded", "true");
  (posItems.find((i) => i.dataset.value === position) || posItems[0]).focus();
}

function closePosMenu(refocus) {
  if (posMenu.hidden) return;
  posMenu.hidden = true;
  posButton.setAttribute("aria-expanded", "false");
  if (refocus) posButton.focus();
}

posButton.addEventListener("click", () => (posMenu.hidden ? openPosMenu() : closePosMenu(true)));
posButton.addEventListener("keydown", (e) => {
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    openPosMenu();
  }
});
posItems.forEach((item) =>
  item.addEventListener("click", () => {
    closePosMenu(false);
    setPosition(item.dataset.value);
    $("word").focus();
  }),
);
posMenu.addEventListener("keydown", (e) => {
  const at = posItems.indexOf(document.activeElement);
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    const step = e.key === "ArrowDown" ? 1 : -1;
    posItems[(at + step + posItems.length) % posItems.length].focus();
  } else if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    document.activeElement.click();
  } else if (e.key === "Escape") {
    e.preventDefault();
    closePosMenu(true);
  } else if (e.key === "Tab") {
    closePosMenu(false);
  }
});
document.addEventListener("pointerdown", (e) => {
  if (!posMenu.hidden && !e.target.closest(".slot.pick")) closePosMenu(false);
});
$("exact").addEventListener("change", checkPattern);
$("word").addEventListener("keydown", (e) => {
  if (e.key !== "Enter") return;
  e.preventDefault();
  $("generate").click();
});

function checkPattern() {
  clearTimeout(checkTimer);
  const p = readPattern();
  if (!p.prefix && !p.suffix && !p.contains) {
    renderEstimate(null);
    return;
  }
  const v = validate_pattern(p.chain, p.prefix, p.suffix, p.ignoreCase, p.contains || undefined);
  if (!v.ok) {
    renderEstimate(null, v.error);
    return;
  }
  const measured = state.speeds.get(speedKey(p));
  showEstimate(p, measured);
  if (!measured) measureSpeed(p);
}

function showEstimate(p, measured) {
  try {
    state.estimate = estimate_difficulty(p.chain, p.prefix, p.suffix, p.ignoreCase, measured || 0, p.contains || undefined);
    state.estimate.measured = Boolean(measured);
    renderEstimate(state.estimate);
  } catch (e) {
    renderEstimate(null, e.message || String(e));
  }
}

async function measureSpeed(p) {
  const token = ++benchToken;
  state.bench?.stop();
  const startedAt = performance.now();
  let foundAfter = Infinity;
  const pool = startPool(p, () => (foundAfter = performance.now() - startedAt));
  state.bench = pool;
  await sleep(BENCH_MS);
  const rate = pool.rate();
  pool.stop();
  if (token !== benchToken || !rate) return;
  state.bench = null;
  // A match during warm-up means the threads barely ran, so the rate would be far too low.
  if (foundAfter < WARMUP_MS) {
    if (state.estimate && mode === "setup") {
      state.estimate.quick = true;
      renderEstimate(state.estimate);
    }
    return;
  }
  state.speeds.set(speedKey(p), rate);
  const now = readPattern();
  if (speedKey(now) === speedKey(p) && mode === "setup") checkPattern();
}

function renderEstimate(est, error) {
  const meta = $("meta");
  const err = $("patternError");
  err.hidden = !error;
  err.textContent = error || "";
  $("generate").disabled = !est || est.risk === "impractical";
  meta.className = "meta";
  meta.replaceChildren();
  if (!est) {
    if (!error) {
      meta.append(
        `${state.chain.hint.replace(/[.\s]*$/, ".")} Type a word above. Use `,
        el("code", "", "*"),
        " for any character, or commas for options like ",
        el("code", "", "cafe,face"),
        ".",
      );
    }
    return;
  }
  meta.classList.add("estimate", est.risk);
  const time =
    est.risk === "impractical" ? "Not practical on one device." : `${capitalize(est.time_label)} on average.`;
  meta.append(el("strong", "", time));
  const source = est.measured ? "measured here" : est.quick ? "rough guess" : "measuring…";
  const detail =
    est.risk === "impractical"
      ? ` Expected wait: ${est.time_label.replace(/ —.*/, "")}. Shorten the word until the estimate is hours, not years.`
      : ` ${est.attempts_label} tries at ${formatSpeed(est.keys_per_sec)} keys/sec (${source}).${
          est.risk === "none" ? "" : ` ${est.guide_rule}.`
        }`;
  meta.append(detail);
}

$("generate").addEventListener("click", () => {
  // Re-check now: the debounced check may not have run for the latest keystroke yet.
  checkPattern();
  if (!state.estimate || $("generate").disabled) return;
  startGenerating(readPattern());
});

/* ---------------- worker pool ---------------- */

function threadCount() {
  const cores = navigator.hardwareConcurrency || 4;
  return Math.max(1, Math.min(16, cores - 1));
}

function startPool(job, onFound, onError) {
  const workers = [];
  let attempts = 0;
  let stopped = false;
  let stoppedAt = 0;
  const started = performance.now();

  const stop = () => {
    if (!stopped) stoppedAt = performance.now();
    stopped = true;
    workers.forEach((w) => w.terminate());
    workers.length = 0;
  };

  for (let i = 0; i < threadCount(); i++) {
    const w = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
    w.onmessage = (e) => {
      if (stopped) return;
      const msg = e.data;
      if (msg.type === "progress") attempts += msg.attempts;
      else if (msg.type === "found") {
        attempts += msg.attempts;
        stop();
        onFound(msg.result, attempts);
      } else if (msg.type === "error") {
        stop();
        onError?.(msg.message);
      }
    };
    w.onerror = (e) => {
      if (stopped) return;
      stop();
      onError?.(e.message || "A worker failed to start.");
    };
    w.postMessage({ module: state.module, job });
    workers.push(w);
  }

  return {
    stop,
    attempts: () => attempts,
    rate: () => attempts / Math.max(0.001, ((stoppedAt || performance.now()) - started) / 1000),
  };
}

/* ---------------- generating ---------------- */

let tick = 0;
let reelTick = 0;

function startGenerating(p) {
  benchToken++;
  state.bench?.stop();
  state.bench = null;
  const expected = state.estimate.attempts;
  $("runMeta").textContent = `Starting… about ${state.estimate.time_label} on average.`;
  $("runNote").textContent = "Each try is a fresh coin flip, so a match can come early or late. Keep this tab open.";
  $("meterFill").style.setProperty("width", "0%");
  setMode("running");
  announce("Generating. This tab is searching for your address.");
  board.setAttribute("aria-hidden", "true");

  state.pool = startPool(
    p,
    (result, attempts) => finish(p, result, attempts),
    (message) => {
      stopGenerating();
      setMode("setup");
      renderEstimate(state.estimate, message);
    },
  );

  const startedAt = performance.now();
  state.startedAt = startedAt;
  tick = setInterval(() => {
    const pool = state.pool;
    if (!pool) return;
    const tried = pool.attempts();
    const rate = pool.rate();
    const share = tried / Math.max(1, expected);
    let avg = state.estimate.time_label;
    if (performance.now() - startedAt > 1500 && rate > 0) {
      avg = formatDuration(expected / rate);
      state.speeds.set(speedKey(p), rate);
    }
    $("runMeta").replaceChildren(
      el("strong", "", `${formatCount(tried)} tried`),
      ` at ${formatSpeed(rate)} keys/sec. Average wait ${avg}.`,
    );
    $("meterFill").style.setProperty("width", `${Math.min(100, share * 100).toFixed(1)}%`);
    if (share > 1) {
      $("runNote").textContent = "Past the average — that's normal with random search. The next try is as likely as the first.";
    }
  }, 250);

  reelHead = 0;
  const plate = $("plate");
  syncSweep();
  plate.classList.remove("scanning");
  void plate.offsetWidth; // restart the beam so it lines up with the tile wave
  plate.classList.add("scanning");
  reelTick = setInterval(renderReel, REEL_MS);
  renderReel();
}

// Decorative only: a wave of flips sweeps the free tiles column by column, in the shape of a real address.
function renderReel() {
  const { chars } = previewLayout();
  const fixed = state.chain.fixed;
  const pool = [...state.chain.sample].filter((_, i) => !fixed[i]);
  const span = boardCols + WAVE_TAIL;
  const head = reelHead;
  reelHead = (reelHead + WAVE_STEP) % span;
  const out = chars.map((ch, i) => {
    if (state.locked[i] || fixed[i]) return ch;
    const behind = head - (i % boardCols);
    const inWave = behind >= 0 && behind < WAVE_TAIL;
    if (!inWave && Math.random() > 0.06) return boardTiles[i]?.textContent || ch;
    return pool[(Math.random() * pool.length) | 0];
  });
  paintTiles(out, state.locked, fixed);
}

function stopGenerating() {
  clearInterval(tick);
  clearInterval(reelTick);
  $("plate").classList.remove("scanning");
  state.pool?.stop();
  state.pool = null;
  board.removeAttribute("aria-hidden");
}

$("stop").addEventListener("click", () => {
  stopGenerating();
  setMode("setup");
  renderPreview();
  announce("Stopped.");
});

/* ---------------- result ---------------- */

function finish(p, result, attempts) {
  const elapsed = (performance.now() - state.startedAt) / 1000;
  stopGenerating();
  state.result = { ...result, chain: state.chain.name, pattern: p };
  state.saved = false;
  $("foundMeta").replaceChildren(
    el("strong", "", "Found it."),
    ` ${formatCount(attempts)} tries in ${formatDuration(elapsed)}.`,
  );
  renderSecrets(result.exports);
  setMode("done");
  renderAddress(result.address, p);
  announce(`Found it. Your address is ${result.address}.`);
}

function matchMarks(address, p) {
  const marks = new Array(address.length).fill(false);
  const lower = p.ignoreCase ? address.toLowerCase() : address;
  const norm = (s) => (p.ignoreCase ? s.toLowerCase() : s);
  const mark = (start, len) => {
    for (let i = start; i < start + len && i < marks.length; i++) marks[i] = true;
  };
  const plain = (s) => s && !/[*,]/.test(s);
  if (plain(p.prefix)) {
    const at = lower.indexOf(norm(p.prefix));
    if (at !== -1 && at < 12) mark(at, p.prefix.length);
  }
  if (plain(p.suffix) && lower.endsWith(norm(p.suffix))) mark(address.length - p.suffix.length, p.suffix.length);
  if (plain(p.contains)) {
    const at = lower.indexOf(norm(p.contains));
    if (at !== -1) mark(at, p.contains.length);
  }
  return marks;
}

function renderAddress(address, p) {
  const chars = [...address];
  paintTiles(chars, matchMarks(address, p), null);
  board.classList.add("found");
  boardTiles.forEach((t, i) => t.style.setProperty("--i", String(i)));
  board.setAttribute("aria-label", `Your ${state.chain.name} address: ${address}`);
}

function renderSecrets(exports) {
  const box = $("secrets");
  box.replaceChildren();

  const addr = el("div", "secret");
  const addrHead = el("div", "secret-head");
  addrHead.append(el("span", "secret-label", "Address"));
  const copyAddr = miniButton("Copy", async () => {
    try {
      await navigator.clipboard.writeText(state.result.address);
      copyAddr.textContent = "Copied";
      setTimeout(() => (copyAddr.textContent = "Copy"), 1500);
    } catch {
      copyAddr.textContent = "Copy failed";
    }
  });
  const addrTools = el("div", "secret-tools");
  addrTools.append(copyAddr);
  addrHead.append(addrTools);
  addr.append(addrHead, el("div", "secret-value", state.result.address));
  const explorers = EXPLORERS[state.chain.id];
  if (explorers && /^[A-Za-z0-9:_-]+$/.test(state.result.address)) {
    addr.append(explorerLinks(explorers, state.result.address));
  }
  box.append(addr);

  for (const item of exports) {
    const card = el("div", "secret");
    const head = el("div", "secret-head");
    const tools = el("div", "secret-tools");
    const value = el("div", "secret-value hidden", item.value);

    const reveal = miniButton("Show", () => {
      const hidden = value.classList.toggle("hidden");
      reveal.textContent = hidden ? "Show" : "Hide";
    });
    const copy = miniButton("Copy", async () => {
      try {
        await navigator.clipboard.writeText(item.value);
        state.saved = true;
        copy.textContent = "Copied";
        setTimeout(() => (copy.textContent = "Copy"), 1500);
      } catch {
        copy.textContent = "Copy failed";
      }
    });
    tools.append(reveal, copy);
    head.append(el("span", "secret-label", item.label), tools);
    card.append(head, value);
    if (item.hint) card.append(el("p", "secret-hint", item.hint));
    box.append(card);
  }
}

function explorerLinks(explorers, address) {
  const wrap = el("div", "explorers");
  wrap.append(el("span", "explorers-label", "Check on"));
  for (const [name, icon, base] of explorers) {
    const a = el("a", "explorer");
    a.href = base + address;
    a.target = "_blank";
    a.rel = "noopener noreferrer";
    a.setAttribute("aria-label", `View this address on ${name} (opens in a new tab)`);
    const img = el("img");
    img.src = icon;
    img.alt = "";
    img.width = 16;
    img.height = 16;
    a.append(img, el("span", "", name));
    wrap.append(a);
  }
  return wrap;
}

function miniButton(text, onClick) {
  const b = el("button", "mini", text);
  b.type = "button";
  b.addEventListener("click", onClick);
  return b;
}

$("download").addEventListener("click", () => {
  const r = state.result;
  if (!r) return;
  const lines = [
    "vanity-address — generated in your browser",
    "",
    `Chain:   ${r.chain}`,
    `Address: ${r.address}`,
    "",
    ...r.exports.map((e) => `${e.label}: ${e.value}${e.hint ? `\n  (${e.hint})` : ""}`),
    "",
    `Created: ${new Date().toISOString()}`,
    "Keep this file private. Anyone with the private key controls the funds.",
    "",
  ];
  const blob = new Blob([lines.join("\n")], { type: "text/plain" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `vanity-${r.address.replace(/[^A-Za-z0-9]/g, "").slice(0, 10)}.txt`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  state.saved = true;
});

function wipe() {
  state.result = null;
  state.saved = false;
  $("secrets").replaceChildren();
  $("foundMeta").textContent = "";
  board.classList.remove("found");
}

function confirmLeave() {
  if (state.saved) return Promise.resolve(true);
  const dialog = $("confirmErase");
  dialog.returnValue = "";
  dialog.showModal();
  return new Promise((resolve) => {
    dialog.addEventListener("close", () => resolve(dialog.returnValue === "erase"), { once: true });
  });
}

$("again").addEventListener("click", async () => {
  if (!(await confirmLeave())) return;
  wipe();
  setMode("setup");
  renderPreview();
  checkPattern();
});
$("wipe").addEventListener("click", async () => {
  if (!(await confirmLeave())) return;
  wipe();
  $("word").value = "";
  setMode("setup");
  renderPreview();
  checkPattern();
  announce("Erased.");
});

window.addEventListener("beforeunload", (e) => {
  if (state.result && !state.saved) e.preventDefault();
});

/* ---------------- formatting ---------------- */

function formatSpeed(n) {
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`;
  return String(Math.round(n));
}

function formatCount(n) {
  return Math.round(n).toLocaleString("en-US");
}

function formatDuration(secs) {
  if (!isFinite(secs)) return "—";
  if (secs < 1) return "under a second";
  if (secs < 60) return `${Math.round(secs)} sec`;
  if (secs < 3600) return `${Math.round(secs / 60)} min`;
  if (secs < 86400) return `${(secs / 3600).toFixed(1)} hours`;
  if (secs < 86400 * 365) return `${Math.round(secs / 86400)} days`;
  return `${Math.round(secs / (86400 * 365))} years`;
}

function capitalize(s) {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

/* ---------------- boot ---------------- */

async function loadEngine() {
  const url = new URL("./wasm/vanity_wasm_bg.wasm", import.meta.url);
  const res = await fetch(url);
  try {
    return await WebAssembly.compileStreaming(res.clone());
  } catch {
    return WebAssembly.compile(await res.arrayBuffer());
  }
}

(async () => {
  try {
    state.module = await loadEngine();
    await init({ module_or_path: state.module });
    state.chains = list_chains();
    $("loading").hidden = true;
    $("composer").hidden = false;
    const first = state.chains.find((c) => c.id === DEFAULT_CHAIN) || state.chains[0];
    pickChain(first.id);
    setMode("setup");
  } catch (e) {
    $("loading").textContent = `The generator couldn't start in this browser: ${e.message || e}. Try a recent Chrome, Firefox, Safari, or Edge.`;
  }
})();
