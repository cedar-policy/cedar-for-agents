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

"""Tests for request generation in the Cedar MCP Schema Generator Python bindings."""

import json

import pytest

from cedar_mcp_schema_generator import (
    RequestGeneratorError,
    generate_request,
    generate_request_or_raise,
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


class TestGenerateRequest:
    def test_basic_request(self):
        result = generate_request(
            STUB,
            TOOLS,
            {"params": {"tool": "read_file", "args": {"path": "/etc/hosts"}}},
            principal_type="User",
            principal_id="alice",
            resource_type="McpServer",
            resource_id="my-server",
        )
        assert result["isOk"] is True
        assert "alice" in result["principal"]
        assert "read_file" in result["action"]
        assert "my-server" in result["resource"]
        assert result["entitiesJson"] is not None

    def test_request_with_string_input(self):
        input_json = json.dumps(
            {"params": {"tool": "read_file", "args": {"path": "/tmp"}}}
        )
        result = generate_request(
            STUB,
            TOOLS,
            input_json,
            principal_type="User",
            principal_id="bob",
            resource_type="McpServer",
            resource_id="server1",
        )
        assert result["isOk"] is True
        assert "bob" in result["principal"]

    def test_namespaced_entities(self):
        result = generate_request(
            STUB,
            TOOLS,
            {"params": {"tool": "read_file", "args": {"path": "/tmp"}}},
            principal_type="User",
            principal_id="alice",
            resource_type="McpServer",
            resource_id="s1",
        )
        assert result["isOk"] is True
        assert result["principal"] == 'TestServer::User::"alice"'
        assert "TestServer::Action" in result["action"]
        assert result["resource"] == 'TestServer::McpServer::"s1"'

    def test_entities_json_is_valid(self):
        result = generate_request(
            STUB,
            TOOLS,
            {"params": {"tool": "read_file", "args": {"path": "/tmp"}}},
            principal_type="User",
            principal_id="alice",
            resource_type="McpServer",
            resource_id="s1",
        )
        entities = json.loads(result["entitiesJson"])
        assert isinstance(entities, list)

    def test_invalid_input(self):
        result = generate_request(
            STUB,
            TOOLS,
            "not valid json",
            principal_type="User",
            principal_id="alice",
            resource_type="McpServer",
            resource_id="s1",
        )
        assert result["isOk"] is False
        assert "Invalid tool input" in result["error"]

    def test_invalid_stub(self):
        result = generate_request(
            "invalid schema",
            TOOLS,
            {"params": {"tool": "read_file", "args": {"path": "/tmp"}}},
            principal_type="User",
            principal_id="alice",
            resource_type="McpServer",
            resource_id="s1",
        )
        assert result["isOk"] is False
        assert "Schema error" in result["error"]

    def test_invalid_tools(self):
        result = generate_request(
            STUB,
            "not valid json",
            {"params": {"tool": "read_file", "args": {"path": "/tmp"}}},
            principal_type="User",
            principal_id="alice",
            resource_type="McpServer",
            resource_id="s1",
        )
        assert result["isOk"] is False
        assert "Invalid tool descriptions" in result["error"]

    def test_invalid_config(self):
        result = generate_request(
            STUB,
            TOOLS,
            {"params": {"tool": "read_file", "args": {"path": "/tmp"}}},
            principal_type="User",
            principal_id="alice",
            resource_type="McpServer",
            resource_id="s1",
            config={"badField": True},
        )
        assert result["isOk"] is False
        assert "Invalid config" in result["error"]

    def test_multi_tool_request(self):
        tools = [
            {
                "name": "read_file",
                "description": "Read a file",
                "inputSchema": {
                    "type": "object",
                    "properties": {"path": {"type": "string"}},
                    "required": ["path"],
                },
            },
            {
                "name": "write_file",
                "description": "Write a file",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "content": {"type": "string"},
                    },
                    "required": ["path", "content"],
                },
            },
        ]
        result = generate_request(
            STUB,
            tools,
            {
                "params": {
                    "tool": "write_file",
                    "args": {"path": "/tmp/out", "content": "hello"},
                }
            },
            principal_type="User",
            principal_id="alice",
            resource_type="McpServer",
            resource_id="s1",
        )
        assert result["isOk"] is True
        assert "write_file" in result["action"]

    def test_nested_object_input_produces_entities(self):
        tools = [
            {
                "name": "ingest",
                "description": "Ingest a record",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "metadata": {
                            "type": "object",
                            "properties": {
                                "source": {"type": "string"},
                                "region": {"type": "string"},
                            },
                            "required": ["source"],
                        },
                        "score": {"type": "number"},
                        "note": {"type": "string"},
                    },
                    "required": ["metadata", "score"],
                },
            }
        ]
        result = generate_request(
            STUB,
            tools,
            {
                "params": {
                    "tool": "ingest",
                    "args": {
                        "metadata": {"source": "sensor-42", "region": "us-east"},
                        "score": 0.87,
                        "note": "ok",
                    },
                }
            },
            principal_type="User",
            principal_id="alice",
            resource_type="McpServer",
            resource_id="s1",
        )
        assert result["isOk"] is True
        entities = json.loads(result["entitiesJson"])
        assert len(entities) > 0

    def test_config_with_request(self):
        result = generate_request(
            STUB,
            TOOLS,
            {"params": {"tool": "read_file", "args": {"path": "/tmp"}}},
            principal_type="User",
            principal_id="alice",
            resource_type="McpServer",
            resource_id="s1",
            config={"numbersAsDecimal": True},
        )
        assert result["isOk"] is True


class TestGenerateRequestOrRaise:
    def test_success(self):
        result = generate_request_or_raise(
            STUB,
            TOOLS,
            {"params": {"tool": "read_file", "args": {"path": "/tmp"}}},
            principal_type="User",
            principal_id="alice",
            resource_type="McpServer",
            resource_id="my-server",
        )
        assert result["isOk"] is True
        assert "alice" in result["principal"]

    def test_raises_on_error(self):
        with pytest.raises(RequestGeneratorError, match="Invalid tool input"):
            generate_request_or_raise(
                STUB,
                TOOLS,
                "bad json",
                principal_type="User",
                principal_id="alice",
                resource_type="McpServer",
                resource_id="s1",
            )
