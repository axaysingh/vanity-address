// Offline copy of this page's own files. Never sees keys: it only stores static assets fetched from this site.
const CACHE = "vanity-generate-v1";
const SLOW_MS = 3000;

const ICONS = [
  "ada", "algo", "apt", "atom", "btc", "dash", "doge", "dot", "dydx", "egld", "eth", "fil", "hbar", "icp", "inj",
  "kas", "ksm", "ltc", "near", "osmo", "sei", "sol", "sui", "tia", "ton", "trx", "xlm", "xrp", "xtz",
];

const PRECACHE = [
  "./",
  "app.css",
  "app.js",
  "chain-meta.js",
  "theme.js",
  "worker.js",
  "manifest.webmanifest",
  "wasm/vanity_wasm.js",
  "wasm/vanity_wasm_bg.wasm",
  "app-icons/icon-192.png",
  "app-icons/icon-512.png",
  "app-icons/apple-touch-180.png",
  "../assets/logo.svg",
  ...ICONS.map((t) => `icons/${t}.png`),
];

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(CACHE).then((cache) => cache.addAll(PRECACHE)).then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  );
});

// Network first so an online visitor always runs the latest code; the cache is only the offline fallback.
self.addEventListener("fetch", (event) => {
  const { request } = event;
  if (request.method !== "GET" || new URL(request.url).origin !== self.location.origin) return;
  event.respondWith(fromNetworkElseCache(request));
});

async function fromNetworkElseCache(request) {
  const cache = await caches.open(CACHE);
  const key = request.mode === "navigate" ? "./" : request;
  const network = fetch(request).then((res) => {
    if (res.ok && res.type === "basic") cache.put(key, res.clone());
    return res;
  });
  const cached = await cache.match(key, { ignoreSearch: request.mode === "navigate" });
  if (!cached) return network;
  const slow = new Promise((resolve) => setTimeout(() => resolve(cached), SLOW_MS));
  return Promise.race([network.catch(() => cached), slow]);
}
