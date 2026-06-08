#!/usr/bin/env node
/*
 * Copyright Cedar Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import fs from "node:fs";
import path from "node:path";

const [pkgDirArg, cargoTomlArg] = process.argv.slice(2);

if (!pkgDirArg || !cargoTomlArg) {
  console.error(
    "Usage: node .github/scripts/prepare_wasm_npm_package.mjs <pkg-dir> <Cargo.toml>",
  );
  process.exit(1);
}

const pkgDir = path.resolve(pkgDirArg);
const cargoTomlPath = path.resolve(cargoTomlArg);
const packageJsonPath = path.join(pkgDir, "package.json");

const cargoToml = fs.readFileSync(cargoTomlPath, "utf8");
const versionMatch = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);

if (!versionMatch) {
  console.error(`Could not find package.version in ${cargoTomlPath}`);
  process.exit(1);
}

const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
const cargoVersion = versionMatch[1];
const packageName = "@cedar-policy/mcp-schema-generator-wasm";

// wasm-pack derives scoped package names from the Rust crate name. Since this
// crate already carries the repository prefix, the derived scoped name would be
// @cedar-policy/cedar-policy-mcp-schema-generator-wasm. Normalize it to the
// shorter package name documented for JavaScript consumers.
packageJson.name = packageName;
packageJson.version = cargoVersion;
packageJson.description =
  "WASM bindings for generating Cedar schemas and authorization requests from MCP tool descriptions.";
packageJson.license = "Apache-2.0";
packageJson.repository = {
  type: "git",
  url: "https://github.com/cedar-policy/cedar-for-agents.git",
  directory: "rust/cedar-policy-mcp-schema-generator-wasm",
};
packageJson.homepage =
  "https://github.com/cedar-policy/cedar-for-agents/tree/main/rust/cedar-policy-mcp-schema-generator-wasm";
packageJson.keywords = ["cedar", "authorization", "agents", "mcp", "wasm"];
packageJson.sideEffects = false;

fs.writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`);
console.log(`Prepared ${packageJson.name}@${packageJson.version} in ${pkgDir}`);
