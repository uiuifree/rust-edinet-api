# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `EdinetApi::metadata` and `documents` for the document list API (`documents.json`, type 1 and 2).
- `EdinetApi::download` for the document API, returning ZIP or PDF bytes.
- Typed codes (`DocTypeCode`, `OrdinanceCode`, status enums) with an `Other` fallback, and
  `Date` / `DateTime` types.
- Error detection for EDINET's status 200 error bodies and the gateway's `StatusCode` bodies.
