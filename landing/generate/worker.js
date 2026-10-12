// One grinding thread. Receives the compiled WASM module from the page, so nothing is fetched here.
import init, { grind_chunk } from "./wasm/vanity_wasm.js";

const TARGET_CHUNK_MS = 120;

self.onmessage = async (event) => {
  const { module, job } = event.data;
  try {
    await init({ module_or_path: module });
    grindLoop(job);
  } catch (e) {
    self.postMessage({ type: "error", message: (e && e.message) || String(e) });
  }
};

function grindLoop(job) {
  let chunk = 256;
  for (;;) {
    const started = performance.now();
    const out = grind_chunk(job.chain, job.prefix, job.suffix, job.ignoreCase, chunk, job.contains || undefined);
    const took = performance.now() - started;

    if (out.found) {
      self.postMessage({ type: "found", attempts: out.attempts, result: out.result });
      return;
    }
    self.postMessage({ type: "progress", attempts: out.attempts });

    // Keep each chunk near TARGET_CHUNK_MS so progress stays smooth on slow and fast chains alike.
    const scale = Math.min(4, Math.max(0.25, TARGET_CHUNK_MS / Math.max(took, 1)));
    chunk = Math.max(16, Math.min(200_000, Math.round(chunk * scale)));
  }
}
