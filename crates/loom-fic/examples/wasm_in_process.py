#!/usr/bin/env python3
"""docs/05's "WASM-in-process" embedding path: any language with a WASI-
capable wasm runtime (wasmtime here) can load loom-fic's *plain*
wasm32-unknown-unknown build (no wasm-bindgen glue -- that's JS-specific;
this target is built straight from `cargo build --target
wasm32-unknown-unknown`, see roadmap/PHASES.md Phase 4 status) and call the
exact same C ABI shim (src/lib.rs) natively-linked callers use. No network
hop, no per-language rewrite of the render logic -- the host only needs to
read the callee's *own* linear memory, since a wasm instance has no shared
address space with its host.

Usage: python3 wasm_in_process.py /path/to/loom_fic.wasm
"""

import sys

from wasmtime import Config, Engine, FuncType, Linker, Module, Store, ValType


def render(wasm_path: str) -> str:
    config = Config()
    engine = Engine(config)
    module = Module.from_file(engine, wasm_path)
    linker = Linker(engine)
    store = Store(engine)

    # `bevy_ecs` (via `bevy_tasks`'s wasm32 async executor) unconditionally
    # links `wasm-bindgen-futures`, even though this crate's own code never
    # calls into it (see lib.rs's doc comment) -- a real, worth-documenting
    # friction point for a *plain* (non-JS) wasm32 host: the four imports
    # below are the only ones a Bevy-ECS-based component actually needs to
    # satisfy instantiation, and this render path never exercises any of
    # them at runtime (no JS interop, no panics thrown through
    # wasm-bindgen's `throw_str`) -- they're stubbed as no-ops purely to
    # let `Linker.instantiate` succeed.
    i32 = ValType.i32()
    linker.define_func("__wbindgen_placeholder__", "__wbindgen_describe", FuncType([i32], []), lambda x: None)
    linker.define_func(
        "__wbindgen_placeholder__",
        "__wbg___wbindgen_throw_344f42d3211c4765",
        FuncType([i32, i32], []),
        lambda ptr, length: None,
    )
    linker.define_func(
        "__wbindgen_externref_xform__",
        "__wbindgen_externref_table_grow",
        FuncType([i32], [i32]),
        lambda delta: 0,
    )
    linker.define_func(
        "__wbindgen_externref_xform__",
        "__wbindgen_externref_table_set_null",
        FuncType([i32], []),
        lambda idx: None,
    )

    instance = linker.instantiate(store, module)

    exports = instance.exports(store)
    loom_fic_render = exports["loom_fic_render"]
    loom_fic_free = exports["loom_fic_free"]
    memory = exports["memory"]

    ptr = loom_fic_render(store)
    length_bytes = memory.read(store, ptr, ptr + 4)
    length = int.from_bytes(length_bytes, byteorder="little")
    html_bytes = memory.read(store, ptr + 4, ptr + 4 + length)
    html = html_bytes.decode("utf-8")

    loom_fic_free(store, ptr, 4 + length)
    return html


if __name__ == "__main__":
    wasm_path = sys.argv[1] if len(sys.argv) > 1 else "./loom_fic.wasm"
    print(render(wasm_path))
