+++
title = "FFI (Foreign Function Interface)"
date = 2026-04-21
description = "How Rust talks to code it did not compile — the vocabulary behind rustwerk's plugin system."

[taxonomies]
tags = ["ffi", "plugin", "glossary"]

[extra]
note_type = "concept"
links = [
  { relation = "relates-to", target = "architecture/crate-plugin-api" },
  { relation = "relates-to", target = "architecture/plugin-host" },
  { relation = "relates-to", target = "architecture/crate-jira-plugin" },
  { relation = "relates-to", target = "decisions/ffi-plugin-boundary" },
]
+++

**FFI** stands for **Foreign Function Interface**. It is
the mechanism by which code written in one language
calls — or is called by — code written in another,
compiled by a different compiler, and potentially
living in a separately-distributed binary.

In rustwerk, "FFI" always refers to **the C ABI**: the
calling convention and data-layout rules the C language
standardized decades ago and which virtually every
operating system, compiler, and language runtime now
understands. Rust opts into the C ABI with two
markers:

- `extern "C"` on a function declares the C calling
  convention (register use, stack layout, name
  mangling).
- `#[repr(C)]` on a struct or enum forces the C memory
  layout so the bytes mean the same thing to both
  sides.

## Why it shows up in rustwerk

Rustwerk plugins are **dynamic libraries**
(`.dll` / `.so` / `.dylib`) that the host loads at
runtime. Because Rust has no stable ABI of its own, a
plugin compiled against a different rustc version
cannot safely share Rust types with the host. The
C ABI is the only contract that stays the same across
compiler versions, languages, and years. So rustwerk
defines its plugin contract as four `extern "C"`
functions that exchange JSON-encoded C strings — the
payload itself is language-neutral, and the boundary
itself is compiler-neutral.

## Why it implies `unsafe`

The C ABI cannot express Rust's borrow-checker
invariants. Any call across the boundary involves raw
pointers, explicit lifetime discipline, and allocator
ownership rules the compiler cannot verify for you.
Rust forces that truth to the surface by requiring
`unsafe` blocks for every call. Rustwerk isolates all
of this into a single module — see
[Plugin host](@/architecture/plugin-host.md) — so the
rest of the codebase keeps its safety guarantees.

## Related

- [Plugin API crate](@/architecture/crate-plugin-api.md) —
  the Rust expression of the C contract.
- [FFI plugin boundary](@/decisions/ffi-plugin-boundary.md) —
  why this shape was chosen over a Rust-trait-object
  plugin system.
