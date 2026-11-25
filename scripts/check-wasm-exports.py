#!/usr/bin/env python3
"""Assert every contract WASM exports exactly its own #[contractimpl] API (issue #1107).

Soroban links the `#[contractimpl]` exports of *every* dependency into the
dependent WASM. A contract that depends on another contract crate therefore
silently ships that crate's entry points, and two `__constructor`s clash so
that neither is exported. This check builds nothing itself: run it after
`cargo build --target <target> --release`.

For each deployable crate (`cdylib`) it compares the function exports of
`target/<target>/release/<crate>.wasm` with the `pub fn`s declared in the
crate's own non-test `#[contractimpl]` blocks, and fails on:
  * extra exports   (leaked from a dependency),
  * missing exports (e.g. a `__constructor` lost to a symbol clash).

Usage: scripts/check-wasm-exports.py [--target wasm32-unknown-unknown]
"""
import argparse
import glob
import os
import re
import sys

# Exports the Soroban toolchain adds that are not contract functions.
IGNORED_EXPORTS = {"_", "memory", "__data_end", "__heap_base"}


# ── WASM parsing ──────────────────────────────────────────────────────────────

def _leb128(buf, pos):
    result = shift = 0
    while True:
        byte = buf[pos]
        pos += 1
        result |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return result, pos
        shift += 7


def wasm_function_exports(path):
    with open(path, "rb") as fh:
        buf = fh.read()
    if buf[:4] != b"\0asm":
        raise ValueError(f"{path}: not a wasm module")
    pos = 8
    exports = []
    while pos < len(buf):
        section_id = buf[pos]
        size, pos = _leb128(buf, pos + 1)
        end = pos + size
        if section_id == 7:  # export section
            count, p = _leb128(buf, pos)
            for _ in range(count):
                name_len, p = _leb128(buf, p)
                name = buf[p:p + name_len].decode("utf-8")
                p += name_len
                kind = buf[p]
                _, p = _leb128(buf, p + 1)
                if kind == 0:  # function
                    exports.append(name)
        pos = end
    return exports


# ── Rust source scanning ──────────────────────────────────────────────────────

def _mask(src):
    """Blank out comments, strings and char literals (keeping newlines)."""
    out = list(src)
    i, n = 0, len(src)

    def blank(a, b):
        for k in range(a, min(b, n)):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j == -1 else j
            blank(i, j)
            i = j
            continue
        if src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            blank(i, j)
            i = j
            continue
        m = re.match(r'b?r(#*)"', src[i:i + 10])
        if m and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            close = '"' + m.group(1)
            end = src.find(close, i + len(m.group(0)))
            end = n if end == -1 else end + len(close)
            blank(i, end)
            i = end
            continue
        if src[i] == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            blank(i, j + 1)
            i = j + 1
            continue
        if src[i] == "'":
            m = re.match(r"'(\\.[^']*|[^\\'])'", src[i:i + 12])
            if m:
                blank(i, i + len(m.group(0)))
                i += len(m.group(0))
                continue
        i += 1
    return "".join(out)


def _close(m, idx, o="{", c="}"):
    depth = 0
    for k in range(idx, len(m)):
        if m[k] == o:
            depth += 1
        elif m[k] == c:
            depth -= 1
            if depth == 0:
                return k
    return len(m)


def contractimpl_fns(src):
    m = _mask(src)
    test_ranges = [
        (t.start(), _close(m, t.end() - 1))
        for t in re.finditer(r"#\[cfg\(test\)\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{", m)
    ]
    names = set()
    for attr in re.finditer(r"#\[contractimpl[^\]]*\]", m):
        if any(a <= attr.start() <= b for a, b in test_ranges):
            continue
        impl = re.compile(r"\s*(?:#\[[^\]]*\]\s*)*impl\b([^{]*)\{").match(m, attr.end())
        if not impl:
            continue
        is_trait = " for " in impl.group(1)
        open_idx = impl.end() - 1
        close_idx = _close(m, open_idx)
        depth = 0
        k = open_idx + 1
        fn_re = re.compile(r"(pub(?:\([^)]*\))?\s+)?fn\s+(\w+)")
        while k < close_idx:
            ch = m[k]
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
            elif depth == 0:
                fm = fn_re.match(m, k)
                if fm and (k == 0 or not (m[k - 1].isalnum() or m[k - 1] == "_")):
                    if fm.group(1) or is_trait:
                        names.add(fm.group(2))
                    k = fm.end()
                    continue
            k += 1
    return names


def crate_expected_exports(crate_dir):
    names = set()
    for path in glob.glob(os.path.join(crate_dir, "src", "**", "*.rs"), recursive=True):
        if "test" in os.path.basename(path):
            continue
        with open(path, encoding="utf-8") as fh:
            names |= contractimpl_fns(fh.read())
    return names


# ── main ──────────────────────────────────────────────────────────────────────

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--target", default="wasm32-unknown-unknown")
    ap.add_argument("--contracts-dir", default="contracts")
    args = ap.parse_args()

    manifests = [os.path.join(args.contracts_dir, "Cargo.toml")]
    manifests += sorted(glob.glob(os.path.join(args.contracts_dir, "*", "Cargo.toml")))
    release_dir = os.path.join("target", args.target, "release")
    failures = 0
    checked = 0
    for manifest in manifests:
        with open(manifest, encoding="utf-8") as fh:
            text = fh.read()
        if "cdylib" not in text:
            continue
        name = re.search(r'^name\s*=\s*"([^"]+)"', text, re.M).group(1)
        wasm = os.path.join(release_dir, name.replace("-", "_") + ".wasm")
        if not os.path.exists(wasm):
            print(f"::error file={manifest}::expected {wasm} to be built")
            failures += 1
            continue
        crate_dir = os.path.dirname(manifest)
        expected = crate_expected_exports(crate_dir)
        exported = wasm_function_exports(wasm)
        actual = set(exported) - IGNORED_EXPORTS
        checked += 1

        extra = sorted(actual - expected)
        missing = sorted(expected - actual)
        if exported.count("__constructor") > 1:
            print(f"::error file={wasm}::exports __constructor more than once")
            failures += 1
        if extra:
            print(f"::error file={wasm}::exports not declared by {name}'s own #[contractimpl] "
                  f"(leaked from a dependency): {', '.join(extra)}")
            failures += 1
        if missing:
            print(f"::error file={wasm}::{name} declares but does not export: {', '.join(missing)}"
                  " (a duplicate symbol in a dependency can drop e.g. __constructor)")
            failures += 1

    print(f"Checked exports of {checked} contract WASM file(s).")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
