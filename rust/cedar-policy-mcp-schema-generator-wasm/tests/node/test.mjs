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

// Integration tests exercising the WASM package from Node.js.
// Requires `wasm-pack build --target nodejs` to have been run first.

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { generateSchema, generateRequest } from "../../pkg/cedar_policy_mcp_schema_generator_wasm.js";

const STUB = `
  namespace TestServer {
    @mcp_principal
    entity User;
    @mcp_resource
    entity McpServer;
    action "call_tool" appliesTo {
      principal: [User],
      resource: [McpServer]
    };
  }
`;

const TOOLS = JSON.stringify([
  {
    name: "read_file",
    description: "Read a file from disk",
    inputSchema: {
      type: "object",
      properties: { path: { type: "string" } },
      required: ["path"],
    },
  },
]);

describe("generateSchema", () => {
  it("produces a valid schema from a stub and tool descriptions", () => {
    const result = JSON.parse(generateSchema(STUB, TOOLS));
    assert.equal(result.isOk, true, `Expected success, got error: ${result.error}`);
    assert.ok(result.schema.length > 0, "Schema should not be empty");
    assert.ok(result.schemaJson.length > 0, "Schema JSON should not be empty");
    assert.ok(result.schema.includes("read_file"), "Schema should reference the tool");
  });

  it("returns an error for an invalid stub", () => {
    const result = JSON.parse(generateSchema("not valid cedar", TOOLS));
    assert.equal(result.isOk, false);
    assert.ok(result.error.length > 0);
  });
});

describe("generateRequest", () => {
  it("produces an authorization request for a tool call", () => {
    const input = JSON.stringify({
      params: { tool: "read_file", args: { path: "/etc/hosts" } },
    });

    const result = JSON.parse(
      generateRequest(STUB, TOOLS, input, "User", "alice", "McpServer", "my-server"),
    );
    assert.equal(result.isOk, true, `Expected success, got error: ${result.error}`);
    assert.ok(result.principal.includes("alice"), "Principal should reference alice");
    assert.ok(result.action.includes("read_file"), "Action should reference the tool");
    assert.ok(result.resource.includes("my-server"), "Resource should reference the server");
    assert.ok(result.entitiesJson.length > 0, "Entities JSON should not be empty");
  });

  it("returns an error for a non-existent tool", () => {
    const input = JSON.stringify({
      params: { tool: "no_such_tool", args: {} },
    });

    const result = JSON.parse(
      generateRequest(STUB, TOOLS, input, "User", "alice", "McpServer", "my-server"),
    );
    assert.equal(result.isOk, false);
    assert.ok(result.error.length > 0);
  });
});

describe("package metadata", () => {
  it("has the correct normalized package name and sideEffects flag", () => {
    const pkg = JSON.parse(readFileSync(new URL("../../pkg/package.json", import.meta.url), "utf8"));
    assert.equal(pkg.name, "@cedar-policy/mcp-schema-generator-wasm");
    assert.equal(pkg.sideEffects, false);
  });
});
