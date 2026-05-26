# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- Moved some inner error types of the `err` out of the public API. Callers should only be able to differentiate top-level error kinds and display their representation.

### Fixed
- `PropertyType::Decimal` validation properly rejects inputs such as `1-0.2` and `-01.2` (`-` in the numbers and negative number with leading zeroes).
- JSON parser now decodes JSON escape sequences in JSON strings.
- Malformed `additionalProperties` schemas now return a deserialization error instead of being silently ignored.

## [0.3.0] - 2026-05-12

### Added:
- Adds support for parsing and type validation of requests to (`Input` struct) and responses from (`Output` struct) MCP servers for the `tools/call` procedure.
- Adds translation of tuple definitions in JSON Schema to `PropertyType::Tuple`.

### Changed:
- Types arrays in JSON Schemas are translated to union of types instead of tuples.

### Fixed:
- Fixed parsing of JSON Schema type arrays containing `"object"` or `"array"` (e.g., `"type": ["null", "object"]`). Previously, only primitive types were supported in type arrays.

## [0.2.0] - 2025-12-05

### Added:
- Adds `Unknown` `PropertyType` to represent when a property's (sub-)type is not provided.

### Fixed:
- Fixed a bug in MCP Tool description parser that would reject legal JSON schemas when the schemas included
array and property types that are unspecified.

## [0.1.0] - 2025-11-11
- Initial release of `mcp-tools-sdk`.
