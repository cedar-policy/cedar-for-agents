# Cedar For Agents

![Cedar Logo](./logo.svg)

![nightly](https://github.com/cedar-policy/cedar-for-agents/actions/workflows/nightly_build.yml/badge.svg)

This repository contains the source code for software at the intersection of [cedar](https://github.com/cedar-policy/cedar) and Agents.

## Table of Contents
This repository contains a number of directories:

* [rust](./rust/) which contains the source code for a number of Rust crates that provide functionality to secure Agentic workflows using Cedar.
  - [mcp-tools-sdk](./rust/mcp-tools-sdk/) : A crate for parsing and manipulating MCP tool descriptions and data.
  - [cedar-policy-mcp-schema-generator](./rust/cedar-policy-mcp-schema-generator/) : A crate for auto-generating a Cedar Schema for an MCP Server's tool descriptions.
* [js](./js/) which contains JavaScript packages that enable Agents to make use of Cedar and its Analysis Capabilities.
  - [cedar-analysis-mcp-server](./js/cedar-analysis-mcp-server) : A package that creates an MCP server that exposes an interface for Agents to use [Cedar's analysis capabilities](https://github.com/cedar-policy/cedar-spec/tree/main/cedar-lean-cli#analysis).

## Security

See [SECURITY](SECURITY.md) for more information.

## Contributing

We welcome contributions from the community. Please either file an issue, or see [CONTRIBUTING](CONTRIBUTING.md)

## License

This project is licensed under the Apache-2.0 License.
