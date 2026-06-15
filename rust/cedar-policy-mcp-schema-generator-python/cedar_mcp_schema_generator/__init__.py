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

"""Cedar MCP Schema Generator - Python bindings.

Generate Cedar authorization schemas and requests from MCP tool descriptions.
"""

from __future__ import annotations

import json
from typing import Any

from cedar_mcp_schema_generator._native import (
    generate_request as _generate_request,
    generate_schema as _generate_schema,
)


class SchemaGeneratorError(Exception):
    """Raised when schema generation fails."""


class RequestGeneratorError(Exception):
    """Raised when request generation fails."""


def generate_schema(
    schema_stub: str,
    tools: str | list[dict[str, Any]],
    *,
    config: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Generate a Cedar schema from a stub and MCP tool descriptions.

    Args:
        schema_stub: A Cedar schema stub string with @mcp_principal and
            @mcp_resource annotated entity types.
        tools: MCP tool descriptions as a JSON string or list of dicts
            (the `tools` array from an MCP tools/list response).
        config: Optional configuration dict with keys:
            - includeOutputs (bool): Include tool outputs in schema context.
            - objectsAsRecords (bool): Encode objects as Cedar records.
            - eraseAnnotations (bool): Remove mcp annotations from output.
            - flattenNamespaces (bool): Flatten nested namespaces.
            - numbersAsDecimal (bool): Encode numbers as Cedar decimals.
            - deduplicateEntityTypes (bool): Deduplicate equivalent enum types.

    Returns:
        A dict with keys:
            - isOk (bool): Whether generation succeeded.
            - schema (str | None): Generated schema in .cedarschema format.
            - schemaJson (str | None): Generated schema as JSON.
            - error (str | None): Error message if generation failed.

    Raises:
        SchemaGeneratorError: If generation fails and you prefer exceptions
            over checking isOk. Use generate_schema_or_raise() for this.
    """
    tools_json = json.dumps(tools) if isinstance(tools, list) else tools
    config_json = json.dumps(config) if config else None
    return json.loads(_generate_schema(schema_stub, tools_json, config_json))


def generate_schema_or_raise(
    schema_stub: str,
    tools: str | list[dict[str, Any]],
    *,
    config: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Like generate_schema but raises SchemaGeneratorError on failure."""
    result = generate_schema(schema_stub, tools, config=config)
    if not result["isOk"]:
        raise SchemaGeneratorError(result.get("error", "Unknown error"))
    return result


def generate_request(
    schema_stub: str,
    tools: str | list[dict[str, Any]],
    input: str | dict[str, Any],
    *,
    principal_type: str,
    principal_id: str,
    resource_type: str,
    resource_id: str,
    config: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Generate a Cedar authorization request from an MCP tool call.

    Args:
        schema_stub: A Cedar schema stub string.
        tools: MCP tool descriptions as a JSON string or list of dicts.
        input: MCP tool call input as a JSON string or dict. Format:
            {"params": {"tool": "tool_name", "args": {"key": "value"}}}.
        principal_type: The Cedar entity type for the principal (e.g., "User").
        principal_id: The principal identifier (e.g., "alice").
        resource_type: The Cedar entity type for the resource (e.g., "McpServer").
        resource_id: The resource identifier (e.g., "my-server").
        config: Optional configuration dict (same keys as generate_schema).

    Returns:
        A dict with keys:
            - isOk (bool): Whether generation succeeded.
            - principal (str | None): Cedar EntityUID string.
            - action (str | None): Cedar action EntityUID string.
            - resource (str | None): Cedar resource EntityUID string.
            - entitiesJson (str | None): Entities as a JSON array string.
            - error (str | None): Error message if generation failed.
    """
    tools_json = json.dumps(tools) if isinstance(tools, list) else tools
    input_json = json.dumps(input) if isinstance(input, dict) else input
    config_json = json.dumps(config) if config else None
    return json.loads(
        _generate_request(
            schema_stub,
            tools_json,
            input_json,
            principal_type,
            principal_id,
            resource_type,
            resource_id,
            config_json,
        )
    )


def generate_request_or_raise(
    schema_stub: str,
    tools: str | list[dict[str, Any]],
    input: str | dict[str, Any],
    *,
    principal_type: str,
    principal_id: str,
    resource_type: str,
    resource_id: str,
    config: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Like generate_request but raises RequestGeneratorError on failure."""
    result = generate_request(
        schema_stub,
        tools,
        input,
        principal_type=principal_type,
        principal_id=principal_id,
        resource_type=resource_type,
        resource_id=resource_id,
        config=config,
    )
    if not result["isOk"]:
        raise RequestGeneratorError(result.get("error", "Unknown error"))
    return result


__all__ = [
    "generate_schema",
    "generate_schema_or_raise",
    "generate_request",
    "generate_request_or_raise",
    "SchemaGeneratorError",
    "RequestGeneratorError",
]
