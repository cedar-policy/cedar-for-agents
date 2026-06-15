# Copyright Cedar Contributors
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      https://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""Tests for the Cedar MCP Schema Generator Python bindings."""

import json

import pytest

from cedar_mcp_schema_generator import (
    SchemaGeneratorError,
    generate_schema,
    generate_schema_or_raise,
)

STUB = """
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
"""

TOOLS = [
    {
        "name": "read_file",
        "description": "Read a file from disk",
        "inputSchema": {
            "type": "object",
            "properties": {"path": {"type": "string"}},
            "required": ["path"],
        },
    }
]


class TestGenerateSchema:
    def test_basic_schema_generation(self):
        result = generate_schema(STUB, TOOLS)
        assert result["isOk"] is True
        assert result["error"] is None
        assert "read_file" in result["schema"]
        assert result["schemaJson"] is not None

    def test_schema_json_is_valid(self):
        result = generate_schema(STUB, TOOLS)
        schema_json = json.loads(result["schemaJson"])
        assert isinstance(schema_json, dict)

    def test_tools_as_json_string(self):
        result = generate_schema(STUB, json.dumps(TOOLS))
        assert result["isOk"] is True
        assert "read_file" in result["schema"]

    def test_empty_tools(self):
        result = generate_schema(STUB, [])
        assert result["isOk"] is True

    def test_invalid_stub(self):
        result = generate_schema("not a valid schema", TOOLS)
        assert result["isOk"] is False
        assert result["error"] is not None

    def test_invalid_tools_json(self):
        result = generate_schema(STUB, "not valid json")
        assert result["isOk"] is False
        assert "Invalid tool descriptions" in result["error"]

    def test_invalid_config(self):
        result = generate_schema(STUB, TOOLS, config={"unknownField": True})
        assert result["isOk"] is False
        assert "Invalid config" in result["error"]

    def test_config_numbers_as_decimal(self):
        tools = [
            {
                "name": "calculate",
                "description": "Perform calculation",
                "inputSchema": {
                    "type": "object",
                    "properties": {"value": {"type": "number"}},
                },
            }
        ]
        result = generate_schema(
            STUB, tools, config={"numbersAsDecimal": True, "includeOutputs": False}
        )
        assert result["isOk"] is True
        assert result["schema"] is not None

    def test_config_flatten_namespaces(self):
        result = generate_schema(STUB, TOOLS, config={"flattenNamespaces": True})
        assert result["isOk"] is True

    def test_config_all_options(self):
        result = generate_schema(
            STUB,
            TOOLS,
            config={
                "includeOutputs": True,
                "objectsAsRecords": True,
                "eraseAnnotations": False,
                "flattenNamespaces": True,
                "numbersAsDecimal": True,
                "deduplicateEntityTypes": True,
            },
        )
        assert result["isOk"] is True

    def test_multi_tool(self):
        tools = [
            {
                "name": "search",
                "description": "Search for items",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"},
                        "limit": {"type": "integer"},
                    },
                    "required": ["query"],
                },
            },
            {
                "name": "get_item",
                "description": "Get a specific item",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "include_metadata": {"type": "boolean"},
                    },
                    "required": ["id"],
                },
            },
        ]
        result = generate_schema(STUB, tools)
        assert result["isOk"] is True
        assert "search" in result["schema"]
        assert "get_item" in result["schema"]

    def test_nested_object_tool(self):
        tools = [
            {
                "name": "create_record",
                "description": "Create a record",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "metadata": {
                            "type": "object",
                            "properties": {
                                "created_by": {"type": "string"},
                                "priority": {"type": "integer"},
                            },
                        },
                    },
                    "required": ["name"],
                },
            }
        ]
        result = generate_schema(STUB, tools)
        assert result["isOk"] is True

    def test_array_property(self):
        tools = [
            {
                "name": "process_batch",
                "description": "Process items",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "items": {"type": "array", "items": {"type": "string"}}
                    },
                    "required": ["items"],
                },
            }
        ]
        result = generate_schema(STUB, tools)
        assert result["isOk"] is True
        assert "process_batch" in result["schema"]


class TestGenerateSchemaOrRaise:
    def test_success(self):
        result = generate_schema_or_raise(STUB, TOOLS)
        assert result["isOk"] is True
        assert "read_file" in result["schema"]

    def test_raises_on_error(self):
        with pytest.raises(SchemaGeneratorError, match="failed to parse"):
            generate_schema_or_raise("invalid", TOOLS)
