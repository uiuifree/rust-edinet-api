# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-11-11

### Changed

- The `*_flag` fields of `Document` are `Option<bool>`. `null` or a missing flag is `None`
  instead of `false`, so it is not mistaken for "no file".
- `EdinetApi::metadata` returns `MetadataResponse` (with `metadata` inside) instead of `Metadata`.

### Added

- `extra` on every response struct keeps fields EDINET adds later, and serializing gives them back.

## [0.1.0] - 2026-11-11

### Added

- `EdinetApi::metadata` and `documents` for the document list API (`documents.json`, type 1 and 2).
- `EdinetApi::download` for the document API, returning ZIP or PDF bytes.
- Typed codes (`DocTypeCode`, `OrdinanceCode`, status enums) with an `Other` fallback, and
  `Date` / `DateTime` types.
- `edinet` command line tool behind the `cli` feature: `metadata`, `list` with filters,
  `download` and `codes`, JSON output and exit codes by error kind.
- Error detection for EDINET's status 200 error bodies and the gateway's `StatusCode` bodies.
