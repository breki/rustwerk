+++
title = "Persistence Layer"
date = 2026-04-21
description = "A JSON file on disk, versioned by git."

[taxonomies]
tags = ["persistence", "json", "git"]

[extra]
note_type = "architecture"
links = [
  { relation = "part-of", target = "architecture/crate-rustwerk" },
  { relation = "implements", target = "decisions/git-native-json-store" },
  { relation = "relates-to", target = "architecture/domain-layer" },
]
+++

`src/persistence/` is intentionally tiny:

```rust
pub fn serialize_project(p: &Project)   -> Result<String, _>;
pub fn deserialize_project(j: &str)     -> Result<Project, _>;
pub mod file_store; // load/save a Project to/from .rustwerk/project.json
```

The entire project lives in a single JSON file under
`.rustwerk/project.json`, pretty-printed so that diffs
are human-reviewable. `BTreeMap` is used throughout the
domain model to guarantee stable ordering, which in
turn produces stable JSON — essential for clean
`git diff` output.

See the full specification in
[docs/project-file-spec.md](https://github.com/breki/rustwerk/blob/main/docs/project-file-spec.md).
