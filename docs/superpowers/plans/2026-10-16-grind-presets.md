# Grind performance presets — Implementation Plan

> **For agentic workers:** Native execution in-session. Steps use checkbox syntax.

**Goal:** Users pick a named power preset (Eco → Extreme) so CLI, desktop, and web size the grinder consistently via `vanity-core`.

**Architecture:** `GrindPreset` in `vanity-core` maps to worker threads on `SystemProfile`. Frontends pass a preset id; `--threads` still wins.

**Tech Stack:** Rust (`vanity-core`, CLI, Tauri), TypeScript (desktop), vanilla JS (web)

## Preset table

| Id | UI label | Worker rule |
|----|----------|-------------|
| `eco` | Eco | `max(1, logical / 4)` then mem_cap |
| `balanced` | Balanced | current `compute_worker_threads` (default) |
| `fast` | Fast | all logical CPUs, still mem_cap |
| `extreme` | Extreme | same threads as fast; denser progress interval |

Aliases: `efficient`→eco, `best`/`performance`→fast, `superman`→extreme.

## Tasks

1. Core `GrindPreset` + tests
2. CLI `--preset` + interactive picker
3. Desktop preset UI + persistence
4. Web preset → threadCount
5. Docs / CHANGELOG
