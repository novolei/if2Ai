# QG-001 Pack Verify Hygiene Design

Status: draft
Owner: Staff Systems Architecture
Pack: `QG-001`

## Problem

Recent ACT/AWL Packs pass focused tests and builds, but `./scripts/pack verify`
is blocked by unrelated clippy warnings in already-dirty areas. That creates
false negatives for every follow-up Pack.

## Target Behavior

The repo should be able to run the cargo/clippy leg of Pack verification without
known lint-only blockers.

## Scope

This is a no-behavior-change cleanup:

- replace manual membership scans with `.contains`,
- replace unnecessary lazy fallback closures,
- replace `filter_map(bool.then(...))`,
- remove an unused test variable,
- replace `assert_eq!(bool, true)` with `assert!(bool)`.

## Non-Goals

- No product behavior changes.
- No UI changes.
- No MCP/AWL implementation.
- No clippy suppressions.
