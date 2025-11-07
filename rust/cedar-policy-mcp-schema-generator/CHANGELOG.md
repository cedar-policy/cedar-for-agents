# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- MCP server/tool description and related types can no longer be imported from the root namespace. Instead they are availabe from `cedar-policy-mcp-schema-generator::mcp::description::*`.

### Added
- Added command line interface to cedar-policy-mcp-schema-generator.

## [0.2.1] - 2025-11-03

- Fix issue in which schemas were generated record types with `additionalAttributes` set to `true` when the record type represented the shape of an entity type that also had tags. This was in error and causes the generated schema to be malformed.

## [0.2.0] - 2025-10-30

### Added
- Added configuration option to allow flattening namespaces into a single namespace.

### Fixed
 - Fix issue in which mutually recursive type definitions in JSON type schemas would always result in the schema generator returning an error when encoding the type definition.

 ## [0.1.0] - 2025-10-21
 - Initial release of `cedar-policy-mcp-schema-generator`.