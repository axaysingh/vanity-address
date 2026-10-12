import init, { list_chains, chain_details, validate_pattern, estimate_difficulty } from "./wasm/vanity_wasm.js";
import { POPULAR, GROUPS, OTHER_TITLE, META, splitLabel } from "./chain-meta.js";

// No cookies, no network beyond this page's own files. localStorage holds only the theme and the
// notice version below — never keys or addresses.

const $ = (id) => document.getElementById(id);
const BENCH_MS = 1000;
const REEL_MS = window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 900 : 90;

const state = {
  module: null,
  chains: [],
  chain: null, // { id, name, hint, sample }
  speeds: new Map(), // chain id + case mode -> measured keys/sec, this tab only
  estimate: null,
  pool: null,
  bench: null,
  startedAt: 0,
  result: null,
  saved: false,
};

/* ---------------- connection status ---------------- */

function renderNet() {
  const offline = !navigator.onLine;
  $("net").classList.toggle("offline", offline);
  $("net").classList.toggle("online", !offline);
  $("netText").textContent = offline ? "Offline — nothing can leave" : "Online — works offline too";
}
window.addEventListener("online", renderNet);
window.addEventListener("offline", renderNet);
renderNet();

/* ---------------- local preferences ---------------- */

// Bump when the notice text changes so everyone sees the new version once.
const CONSENT_VERSION = "1";

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

window.matchMedia("(prefers-color-scheme: light)").addEventListener("change", (e) => {
  if (readPref("vanity.theme")) return;
  document.documentElement.dataset.theme = e.matches ? "light" : "dark";
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
  focusStep();
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

/* ---------------- steps ---------------- */

let step = 1;

function go(n) {
  step = n;
  for (let i = 1; i <= 4; i++) $(`step${i}`).hidden = i !== n;
  document.querySelectorAll(".rail-step").forEach((li) => {
    const s = Number(li.dataset.step);
    li.classList.toggle("current", s === n);
    li.classList.toggle("done", s < n);
    if (s === n) li.setAttribute("aria-current", "step");
    else li.removeAttribute("aria-current");
  });
  focusStep();
}

function focusStep() {
  if (consent.open) return;
  const h = $(`step${step}`).querySelector("h1");
  h.tabIndex = -1;
  h.focus({ preventScroll: true });
  window.scrollTo({ top: 0 });
}

document.querySelectorAll("[data-back]").forEach((b) =>
  b.addEventListener("click", () => go(Number(b.dataset.back))),
);

/* ---------------- step 1: chain ---------------- */

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function badge(id) {
  const b = el("span", "badge", META[id]?.ticker || id.slice(0, 4).toUpperCase());
  b.setAttribute("aria-hidden", "true");
  return b;
}

function chainButton(c, variant) {
  const { title, detail } = splitLabel(c.name);
  const btn = el("button", variant);
  btn.type = "button";
  btn.dataset.id = c.id;
  btn.style.setProperty("--c", META[c.id]?.color || "#8a8a99");
  if (state.chain?.id === c.id) btn.classList.add("selected");
  btn.setAttribute("aria-label", `${title}${detail ? `, ${detail}` : ""}`);
  const text = el("span", "chain-text");
  text.append(el("span", "chain-title", title));
  if (detail) text.append(el("span", "chain-detail", detail));
  btn.append(badge(c.id), text);
  if (variant === "chain-row") btn.append(el("span", "chain-id", c.id));
  btn.addEventListener("click", () => pickChain(c.id));
  return btn;
}

function matches(c, q) {
  const meta = META[c.id];
  const hay = `${c.name} ${c.id} ${meta?.ticker || ""} ${meta?.words || ""}`.toLowerCase();
  return q.split(/\s+/).every((w) => hay.includes(w));
}

function section(title, count) {
  const head = el("div", "group-head");
  head.append(el("h2", "group-title", title));
  if (count !== undefined) head.append(el("span", "group-count", String(count)));
  return head;
}

function renderChains(filter = "") {
  const list = $("chainList");
  list.replaceChildren();
  const q = filter.trim().toLowerCase();
  const byId = new Map(state.chains.map((c) => [c.id, c]));

  if (q) {
    const hits = state.chains.filter((c) => matches(c, q));
    if (!hits.length) {
      list.append(el("p", "empty", `No chain matches “${filter.trim()}”. Try a name or ticker, like SOL or ETH.`));
      return;
    }
    list.append(section(hits.length === 1 ? "1 match" : `${hits.length} matches`));
    const rows = el("div", "rows");
    hits.forEach((c) => rows.append(chainButton(c, "chain-row")));
    list.append(rows);
    return;
  }

  list.append(section("Popular"));
  const tiles = el("div", "tiles");
  POPULAR.map((id) => byId.get(id))
    .filter(Boolean)
    .forEach((c) => tiles.append(chainButton(c, "chain-tile")));
  list.append(tiles);

  const grouped = new Set(GROUPS.flatMap((g) => g.ids));
  const groups = [
    ...GROUPS.map((g) => ({ title: g.title, items: g.ids.map((id) => byId.get(id)).filter(Boolean) })),
    { title: OTHER_TITLE, items: state.chains.filter((c) => !grouped.has(c.id)) },
  ];
  list.append(section("All chains", state.chains.length));
  for (const g of groups) {
    if (!g.items.length) continue;
    const block = el("div", "group");
    block.append(el("h3", "family", g.title));
    const rows = el("div", "rows");
    g.items.forEach((c) => rows.append(chainButton(c, "chain-row")));
    block.append(rows);
    list.append(block);
  }
}

$("chainSearch").addEventListener("input", (e) => renderChains(e.target.value));
$("chainSearch").addEventListener("keydown", (e) => {
  if (e.key === "Enter") $("chainList").querySelector("button[data-id]")?.click();
  if (e.key === "Escape" && e.target.value) {
    e.target.value = "";
    renderChains();
  }
});
document.addEventListener("keydown", (e) => {
  if (e.key !== "/" || step !== 1 || consent.open) return;
  if (document.activeElement?.matches("input, textarea")) return;
  e.preventDefault();
  $("chainSearch").focus();
});

function pickChain(id) {
  const d = chain_details(id);
  state.chain = { id, name: d.name, hint: d.hint, sample: d.sample };
  $("s2chain").textContent = d.name;
  $("sample").textContent = d.sample;
  $("hint").textContent = d.hint;
  for (const f of ["prefix", "suffix", "contains"]) $(f).value = "";
  $("exact").checked = false;
  // Case only matters where addresses mix upper and lower case (base58, TON).
  $("exact").closest(".check").hidden = !(/[A-Z]/.test(d.sample) && /[a-z]/.test(leadless(d.sample)));
  state.estimate = null;
  renderEstimate(null);
  renderChains($("chainSearch").value);
  go(2);
}

/* ---------------- step 2: pattern ---------------- */

function readPattern() {
  return {
    chain: state.chain.id,
    prefix: $("prefix").value.trim(),
    suffix: $("suffix").value.trim(),
    contains: $("contains").value.trim(),
    ignoreCase: !$("exact").checked,
  };
}

const speedKey = (p) => `${p.chain}:${p.ignoreCase}`;

let checkTimer = 0;
let benchToken = 0;
for (const f of ["prefix", "suffix", "contains", "exact"]) {
  $(f).addEventListener("input", () => {
    clearTimeout(checkTimer);
    checkTimer = setTimeout(checkPattern, 220);
  });
}
["prefix", "suffix", "contains"].forEach((f) =>
  $(f).addEventListener("keydown", (e) => {
    if (e.key !== "Enter") return;
    clearTimeout(checkTimer);
    checkPattern();
    if (!$("toGenerate").disabled) $("toGenerate").click();
  }),
);

function checkPattern() {
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
  const pool = startPool(p, () => {});
  state.bench = pool;
  await sleep(BENCH_MS);
  const rate = pool.rate();
  pool.stop();
  if (token !== benchToken || !rate) return;
  state.bench = null;
  state.speeds.set(speedKey(p), rate);
  const now = readPattern();
  if (speedKey(now) === speedKey(p) && step === 2) checkPattern();
}

function renderEstimate(est, error) {
  const box = $("estimate");
  const err = $("patternError");
  err.hidden = !error;
  err.textContent = error || "";
  $("toGenerate").disabled = !est || est.risk === "impractical";
  if (!est) {
    box.hidden = true;
    return;
  }
  box.hidden = false;
  box.className = `estimate ${est.risk}`;
  $("estTime").textContent =
    est.risk === "impractical" ? "Not practical on one device" : `${capitalize(est.time_label)} on average`;
  const source = est.measured ? "measured on this device" : "rough guess, measuring…";
  $("estMeta").textContent = `${est.attempts_label} tries · ${formatSpeed(est.keys_per_sec)} keys/sec (${source})`;
  $("estRule").textContent =
    est.risk === "impractical"
      ? `Expected wait: ${est.time_label.replace(/ —.*/, "")}. Shorten the pattern until the estimate is hours, not years.`
      : `${est.guide_verdict}. ${est.guide_rule}.`;
}

$("toGenerate").addEventListener("click", () => {
  // Re-check now: the debounced check may not have run for the latest keystroke yet.
  clearTimeout(checkTimer);
  checkPattern();
  if (!state.estimate || $("toGenerate").disabled) return;
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

/* ---------------- step 3: generate ---------------- */

let tick = 0;
let reelTick = 0;

function describeTarget(p) {
  const parts = [];
  if (p.prefix) parts.push(`starting with ${p.prefix}`);
  if (p.suffix) parts.push(`ending with ${p.suffix}`);
  if (p.contains) parts.push(`containing ${p.contains}`);
  return `${state.chain.name} address ${parts.join(" and ")}`;
}

function startGenerating(p) {
  benchToken++;
  state.bench?.stop();
  state.bench = null;
  const expected = state.estimate.attempts;
  $("s3target").textContent = `Looking for a ${describeTarget(p)}.`;
  $("statTried").textContent = "0";
  $("statSpeed").textContent = "—";
  $("statAvg").textContent = state.estimate.time_label;
  $("meterFill").style.width = "0%";
  $("s3note").textContent = "Each try is a fresh coin flip, so a match can come early or late. Keep this tab open.";
  go(3);

  state.pool = startPool(
    p,
    (result, attempts) => finish(p, result, attempts),
    (message) => {
      stopGenerating();
      go(2);
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
    $("statTried").textContent = formatCount(tried);
    $("statSpeed").textContent = `${formatSpeed(rate)}/s`;
    if (performance.now() - startedAt > 1500 && rate > 0) {
      $("statAvg").textContent = formatDuration(expected / rate);
      state.speeds.set(speedKey(p), rate);
    }
    $("meterFill").style.width = `${Math.min(100, share * 100).toFixed(1)}%`;
    if (share > 1) {
      $("s3note").textContent = "Past the average — that's normal with random search. The next try is as likely as the first.";
    }
  }, 250);

  reelTick = setInterval(() => renderReel(), REEL_MS);
  renderReel();
}

// Decorative only: shuffles characters in the shape of a real address while the search runs.
function renderReel() {
  const sample = state.chain.sample;
  const lead = leadOf(sample);
  const pool = sample.slice(lead.length);
  let out = lead;
  for (let i = lead.length; i < sample.length; i++) {
    out += pool[(Math.random() * pool.length) | 0];
  }
  $("reel").textContent = out;
}

function leadless(sample) {
  return sample.slice(leadOf(sample).length);
}

function leadOf(sample) {
  const colon = sample.indexOf(":");
  if (colon !== -1) return sample.slice(0, colon + 1);
  if (sample.startsWith("0x")) return "0x";
  const bech = /^[a-z]+1/.exec(sample);
  if (bech && bech[0].length > 2) return bech[0];
  return sample.slice(0, 1);
}

function stopGenerating() {
  clearInterval(tick);
  clearInterval(reelTick);
  state.pool?.stop();
  state.pool = null;
}

$("stop").addEventListener("click", () => {
  stopGenerating();
  go(2);
});

/* ---------------- step 4: save ---------------- */

function finish(p, result, attempts) {
  const elapsed = (performance.now() - state.startedAt) / 1000;
  stopGenerating();
  state.result = { ...result, chain: state.chain.name, pattern: p };
  state.saved = false;
  renderAddress(result.address, p);
  $("foundMeta").textContent = `Found after ${formatCount(attempts)} tries in ${formatDuration(elapsed)}.`;
  renderSecrets(result.exports);
  go(4);
}

function renderAddress(address, p) {
  const el = $("address");
  el.replaceChildren();
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
  let i = 0;
  while (i < address.length) {
    const hit = marks[i];
    let j = i;
    while (j < address.length && marks[j] === hit) j++;
    const span = document.createElement("span");
    if (hit) span.className = "hit";
    span.textContent = address.slice(i, j);
    el.append(span);
    i = j;
  }
}

function renderSecrets(exports) {
  const box = $("secrets");
  box.replaceChildren();
  for (const item of exports) {
    const card = document.createElement("div");
    card.className = "secret";

    const head = document.createElement("div");
    head.className = "secret-head";
    const label = document.createElement("span");
    label.className = "secret-label";
    label.textContent = item.label;
    const tools = document.createElement("div");
    tools.className = "secret-tools";

    const value = document.createElement("div");
    value.className = "secret-value hidden";
    value.textContent = item.value;

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
    head.append(label, tools);
    card.append(head, value);

    if (item.hint) {
      const hint = document.createElement("p");
      hint.className = "secret-hint";
      hint.textContent = item.hint;
      card.append(hint);
    }
    box.append(card);
  }
}

function miniButton(text, onClick) {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "mini";
  b.textContent = text;
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
  $("address").replaceChildren();
  $("secrets").replaceChildren();
  $("foundMeta").textContent = "";
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
  go(2);
});
$("wipe").addEventListener("click", async () => {
  if (!(await confirmLeave())) return;
  wipe();
  go(1);
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
    renderChains();
    go(1);
  } catch (e) {
    $("loading").textContent = `The generator couldn't start in this browser: ${e.message || e}. Try a recent Chrome, Firefox, Safari, or Edge.`;
  }
})();
