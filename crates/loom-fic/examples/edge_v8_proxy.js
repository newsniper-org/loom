#!/usr/bin/env node
// RQ6 §4.5.2 (M12) — "does a V8-isolate-style edge runtime (Cloudflare
// Workers etc.) load the same plain wasm32 FIC build wasmtime does?" Node.js
// is itself V8, and (like Workers) loads wasm via plain
// `WebAssembly.instantiate` -- so it stands in for that runtime family the
// same way §5.1 already uses a static-HTML host proxy instead of a real
// backend process. This does NOT model Workers' actual CPU-time billing or
// cold-start behavior -- only "does this exact wasm binary instantiate and
// run correctly under a V8 WebAssembly host," which is the structural
// question M12 asks.
//
// Usage: node edge_v8_proxy.js /path/to/loom_fic.wasm

import { readFile } from "node:fs/promises";

const wasmPath = process.argv[2] || "./loom_fic.wasm";
const bytes = await readFile(wasmPath);

// Same four wasm-bindgen-futures/js-sys stub imports Python's wasmtime
// harness needed (examples/wasm_in_process.py) -- this render path never
// actually calls them (see lib.rs's doc comment on bevy_tasks' unconditional
// wasm32 dependency), they only need to exist to satisfy instantiation.
const imports = {
  __wbindgen_placeholder__: {
    __wbindgen_describe: () => {},
    __wbg___wbindgen_throw_344f42d3211c4765: () => {},
  },
  __wbindgen_externref_xform__: {
    __wbindgen_externref_table_grow: () => 0,
    __wbindgen_externref_table_set_null: () => {},
  },
};

const { instance } = await WebAssembly.instantiate(bytes, imports);
const { loom_fic_render, loom_fic_free, memory } = instance.exports;

const ptr = loom_fic_render();
const view = new DataView(memory.buffer);
const len = view.getUint32(ptr, true); // little-endian, matches lib.rs's to_le_bytes()
const htmlBytes = new Uint8Array(memory.buffer, ptr + 4, len);
const html = new TextDecoder("utf-8").decode(htmlBytes);

loom_fic_free(ptr, 4 + len);

console.log(html);
