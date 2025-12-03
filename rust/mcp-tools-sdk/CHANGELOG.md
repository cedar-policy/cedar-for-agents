# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Added:
- Adds `Unknown` `PropertyType` to represent when a property's (sub-)type is not provided.

Fixed:
- Fixed a bug in MCP Tool description parser that would reject legal JSON schemas when the schemas included 
array and property types that are unsecified.

## [0.1.0] - 2025-11-11
- Initial release of `mcp-tools-sdk`.