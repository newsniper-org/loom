#!/usr/bin/env node
// docs/05's Sidecar HTTP embedding path, called from a SECOND language
// (Node.js, vs. the Python callers in native_binding.py/wasm_in_process.py)
// -- proving "polyglot" isn't just "two different Python scripts calling
// two different mechanisms." Any language with an HTTP client works here;
// Node's built-in fetch is the least ceremony.
//
// Usage: node sidecar_client.js http://127.0.0.1:8790/render

const url = process.argv[2] || "http://127.0.0.1:8790/render";

const res = await fetch(url);
const html = await res.text();
console.log(html);
