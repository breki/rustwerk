# Changelog

All notable changes to this project will be documented
in this file.

The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.37.0] - 2026-04-04

### Added

- `task describe <ID>` command reads and displays
  `.rustwerk/tasks/<ID>.md` description files
- `llms.txt` AI-agent reference (llmstxt.org convention)

## [0.36.0] - 2026-04-04

### Added

- `--tag <TAG>` filter for `task list` command

## [0.35.0] - 2026-04-04

### Added

- `--tags` flag for `task add` and `task update`
- `tags` array field for `task.add` and `task.update`
  batch commands

## [0.34.0] - 2026-04-04

### Added

- Tags field on tasks (slug-like, max 20 per task)
- `Tag` newtype with validation
- `add_tag`, `remove_tag`, `has_tag` domain methods

## [0.33.0] - 2026-04-04

### Added

- `dev.add` and `dev.remove` batch commands

## [0.32.0] - 2026-04-04

### Added

- `RUSTWERK_USER` environment variable fallback for
  `task assign` and `effort log`
- Project file format specification (`docs/project-file-spec.md`)

## [0.31.0] - 2026-04-04

### Added

- `status` command for compact project dashboard
- `--remaining` flag for gantt command
- Critical-path highlighting (red bars) in Gantt chart
- ANSI color support with auto-detect and `NO_COLOR`
- `--available` shows TODO tasks, `--active` shows
  IN_PROGRESS
- `ON_HOLD` task status
- Tag support WBS tasks (DOM-TAG, CLI-TAG-SET,
  CLI-TAG-FILTER)
- Developer management WBS tasks (DOM-DEV,
  CLI-DEV-ADD/REMOVE/LIST, DEV-ASSIGN)

### Changed

- Split `scheduling.rs` into focused modules
- Artisan code quality reviewer added to commit workflow
