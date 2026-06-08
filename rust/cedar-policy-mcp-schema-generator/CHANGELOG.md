# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- `deduplicate_entity_types` now consolidates identical leaf record objects into shared common types when `objects_as_records` is enabled, instead of creating duplicate type definitions per tool.

## [0.6.0] - 2026-05-26

### Added
- Adds `deduplicate_entity_types` option to consolidate equivalent enum entity types (same name and variants) and leaf entity types (entities where all attributes are of base type, i.e. no nested entity) into a single definition at the lowest common ancestor namespace.

### Changed
- Translation of JSON numbers to Cedar decimals does not special case integers (represents both `10` and `10.0` as `10.0000`)

### Fixed
- Float entity IDs now use a canonical lossless representation, fixing whole-number floats missing a decimal point and extreme values producing excessively long EIDs.
- In the request generator, property names containing `::` in MCP tool inputs are now rejected. The schema generator already errored on such names; this fix makes the request generator consistent.

## [0.5.0] - 2026-05-12

### Added
- Adds the ability to convert a MCP tool request (and optionally response) to a Cedar request and entity data compliant to the generated Schema.
- Adds support for JSON Schema tuples in MCP tool schemas, which are translated to a record of projections.


### Changed
- Type arrays in MCP tool schemas now result in records of optional fields (encoding union of types), rather than records of projections (encoding tuples).

### Fixed

- Generated JSON format schemas now correctly reference common types using `EntityOrCommon` instead `Entity`.

## [0.4.0] - 2025-12-05

 - Initial release of `cedar-policy-mcp-schema-generator`.
