#!/usr/bin/env python3
"""docs/05's "네이티브 바인딩" embedding path, at its most literal:
loom-fic's C ABI shim (see src/lib.rs) needs no per-language wrapper crate
(no PyO3, no napi-rs) -- any language with a C FFI story can load the
.so/.dll directly. This is exactly the "leo4 C shim" principle: one shim,
N thin per-language callers, none of which redesign anything.

Usage: python3 native_binding.py /path/to/libloom_fic.so
"""

import ctypes
import sys


def render(lib_path: str) -> str:
    lib = ctypes.CDLL(lib_path)
    lib.loom_fic_render.restype = ctypes.POINTER(ctypes.c_ubyte)
    lib.loom_fic_free.argtypes = [ctypes.POINTER(ctypes.c_ubyte), ctypes.c_uint32]

    ptr = lib.loom_fic_render()
    length_bytes = bytes(ptr[0:4])
    length = int.from_bytes(length_bytes, byteorder="little")
    html_bytes = bytes(ptr[4 : 4 + length])
    html = html_bytes.decode("utf-8")

    lib.loom_fic_free(ptr, ctypes.c_uint32(4 + length))
    return html


if __name__ == "__main__":
    lib_path = sys.argv[1] if len(sys.argv) > 1 else "./libloom_fic.so"
    print(render(lib_path))
