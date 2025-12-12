# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Adds the ability to convert a MCP tool request and (optionally response) to a Cedar request and entity data compliant to the generated Schema.

### Fixed

- Generated JSON format schemas now correctly reference common types using `EntityOrCommon` instead `Entity`.

## [0.4.0] - 2025-12-05

 - Initial release of `cedar-policy-mcp-schema-generator`.