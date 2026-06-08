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

const [pkgDirArg] = process.argv.slice(2);

if (!pkgDirArg) {
  console.error(
    "Usage: node .github/scripts/prepare_wasm_npm_package.mjs <pkg-dir>",
  );
  process.exit(1);
}

const pkgDir = path.resolve(pkgDirArg);
const packageJsonPath = path.join(pkgDir, "package.json");

const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));

// wasm-pack derives scoped package names from the Rust crate name. Since this
// crate already carries the repository prefix, the derived scoped name would be
// @cedar-policy/cedar-policy-mcp-schema-generator-wasm. Normalize it to the
// shorter package name documented for JavaScript consumers.
packageJson.name = "@cedar-policy/mcp-schema-generator-wasm";
packageJson.sideEffects = false;

fs.writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`);
console.log(`Prepared ${packageJson.name}@${packageJson.version} in ${pkgDir}`);
