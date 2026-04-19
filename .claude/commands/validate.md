---
description: Run all quality checks with stepwise progress
allowed-tools: Bash(cargo xtask:*)
---

Run the full validation pipeline with concise output.

## Usage

`/validate` -- run clippy + tests + coverage + dupes

## Implementation

```
cargo xtask validate
```
