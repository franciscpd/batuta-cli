---
round: 14
round_created_at: 2026-08-20T23:58:05.278312502Z
status: valid
file: crates/batuta-tui/src/render_cache.rs
line: 311
severity: high
author: unknown
---

# Issue 001: Transcript wrapping ignores Unicode display width

## Review Comment

`wrap_lines` measures both content and indentation with `.chars().count()` and splits by character index. Wide terminal characters such as CJK glyphs occupy two columns, so a line that is considered within the configured width can render beyond it and be clipped; combining characters have the opposite mismatch. This violates the UI contract's requirement to measure wrapping by Unicode display columns and avoid horizontal transcript scrolling. Use `unicode_width::UnicodeWidthStr`/`UnicodeWidthChar` when calculating widths and split points, and add narrow-width coverage with wide and combining Unicode text.

## Triage

- Decision: `VALID`
- Notes: `wrap_lines` uses scalar character counts for its fit check, continuation indentation, and split boundary. Terminal cells instead use Unicode display widths, so a CJK glyph can consume two cells while being counted as one. The workspace already provides `unicode-width`; the complete fix is to use its string and character width APIs for every wrapping decision, retaining a character boundary for safe splitting and adding focused wide/combining-text regression coverage.
