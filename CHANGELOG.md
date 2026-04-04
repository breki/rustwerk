# Changelog

All notable changes to this project will be documented
in this file.

The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
