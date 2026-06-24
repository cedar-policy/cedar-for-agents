# Cedar For Agents

![Cedar Logo](./logo.svg)

![nightly](https://github.com/cedar-policy/cedar-for-agents/actions/workflows/nightly_build.yml/badge.svg)

This repository contains the source code for software at the intersection of [cedar](https://github.com/cedar-policy/cedar) and Agents.

## Table of Contents
This repository contains a number of directories:

* [rust](./rust/) which contains the source code for a number of Rust crates that provide functionality to secure Agentic workflows using Cedar as well bindings in WebAssembly and Python:
  - [mcp-tools-sdk](./rust/mcp-tools-sdk/) : A crate for parsing and manipulating MCP tool descriptions and data.
  - [cedar-policy-mcp-schema-generator](./rust/cedar-policy-mcp-schema-generator/) : A crate for auto-generating a Cedar Schema for an MCP Server's tool descriptions.
  - [cedar-policy-mcp-schema-generator-wasm](./rust/cedar-policy-mcp-schema-generator-wasm/) : WebAssembly bindings for generating Cedar schemas and authorization requests from MCP tool descriptions in JavaScript and TypeScript environments, published as (`@cedar-policy/mcp-schema-generator-wasm)[https://www.npmjs.com/package/@cedar-policy/mcp-schema-generator-wasm] on NPM.
  - [cedar-policy-mcp-schema-generator-python](./rust/cedar-policy-mcp-schema-generator-python/) : PyO3/maturin wrapper of the `cedar-policy-mcp-schema-generator`, published as (`cedar-policy-mcp-schema-generator`)[https://pypi.org/project/cedar-policy-mcp-schema-generator/] on PyPI.
* [js](./js/) which contains JavaScript packages that enable Agents to make use of Cedar and its Analysis Capabilities.
  - [cedar-analysis-mcp-server](./js/cedar-analysis-mcp-server) : A package that creates an MCP server that exposes an interface for Agents to use [Cedar's analysis capabilities](https://github.com/cedar-policy/cedar-spec/tree/main/cedar-lean-cli#analysis).

## Security

See [SECURITY](SECURITY.md) for more information.

## Contributing

We welcome contributions from the community. Please either file an issue, or see [CONTRIBUTING](CONTRIBUTING.md)

## License

This project is licensed under the Apache-2.0 License.
