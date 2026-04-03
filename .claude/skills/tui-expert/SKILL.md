---
name: tui-expert
description: >
  TUI rendering and alignment guidelines for terminal
  output. Use when writing or reviewing code that renders
  aligned columns, bars, charts, or tables to the terminal.
---

# TUI Rendering Guidelines

Hard-won lessons from building and fixing terminal UI
rendering. Apply these whenever writing code that produces
aligned columnar output.

## Display Trait and Format Forwarding

When a newtype wraps a `String` and implements `Display`,
**always forward the formatter** so width/alignment specs
work:

```rust
// WRONG — silently drops width, fill, alignment flags
impl fmt::Display for MyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// RIGHT — forwards all format specs from the caller
impl fmt::Display for MyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
```

Without forwarding, `format!("{:<20}", my_id)` silently
produces an unpadded string. This is the #1 cause of
column misalignment.

## Decorators Consume Body Width

When rendering bars or cells with decorators (brackets,
caps, borders), the decorators must come **from within**
the scaled/budgeted width, not be added on top:

```
WRONG:  scaled_width = 20 → [████████████████████]
        total = 22 chars (overflows budget by 2)

RIGHT:  scaled_width = 20 → [██████████████████]
        body = 18, brackets = 2, total = 20
```

If decorators are added outside the width budget, adjacent
elements will overlap. This applies to:
- Bar chart brackets/caps (`[`, `]`, `▐`, `▌`)
- Table cell borders (`|`, `│`)
- Any fixed-width framing characters

## Byte Offsets vs Character Columns

Rust's `char_indices()` returns **byte** offsets, not
display columns. For multi-byte Unicode characters (all
block-drawing chars are 3 bytes in UTF-8), byte offsets
are 2-3x larger than the display column:

```rust
// WRONG — byte offset, not display column
let col = line.char_indices()
    .find(|(_, c)| *c == '▐')
    .map(|(i, _)| i);  // returns byte offset ~36

// RIGHT — character/display column
let col = line.chars()
    .enumerate()
    .find(|(_, c)| *c == '▐')
    .map(|(i, _)| i);  // returns column ~12
```

Note: this assumes all characters are single-width. For
CJK or emoji (double-width), use the `unicode-width`
crate instead.

## Visual Alignment Tests

`contains()` checks do NOT catch alignment bugs. Always
assert **exact column positions** for aligned output:

```rust
// WRONG — passes even when columns are misaligned
assert!(output.contains("TASK-A"));
assert!(output.contains("▐██▌"));

// RIGHT — asserts the bar starts at a specific column
fn char_col(s: &str, needle: char) -> Option<usize> {
    s.chars().enumerate()
        .find(|(_, c)| *c == needle)
        .map(|(i, _)| i)
}

let col_a = char_col(row_a, '▐').unwrap();
let col_b = char_col(row_b, '▐').unwrap();
assert_eq!(col_a, col_b, "bars must start at same column");
```

Test at minimum:
1. Header ticks align with bar start positions
2. All rows' content starts at the same column
3. Adjacent bars don't overlap (A's end < B's start)
4. Different-length labels produce same column alignment

## Shared Column Budget

Header rows and data rows must use the **exact same**
prefix width. Define it once:

```rust
let label_width = id_width + 2; // marker + id + space
// Header uses label_width for its prefix
print!("{:width$}", "", width = label_width);
// Each row uses the same components
print!("{marker}{:<id_w$} ", row.id, id_w = id_width);
// marker(1) + id(id_width) + space(1) = label_width ✓
```

If the header and body compute their prefix differently,
ticks and bars will be offset.
