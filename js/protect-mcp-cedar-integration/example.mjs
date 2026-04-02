/**
 * Example: Generate a Cedar schema from MCP tool descriptions.
 *
 * This demonstrates how protect-mcp auto-generates typed Cedar
 * authorization schemas from MCP tools/list responses, enabling
 * policies that reference tool input attributes.
 *
 * Run: node example.mjs
 */

import { generateCedarSchema, generateSchemaStub } from 'protect-mcp';

// ── Sample MCP tools (from a typical tools/list response) ──

const tools = [
  {
    name: 'read_file',
    description: 'Read the contents of a file at the given path',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Absolute path to file' },
        encoding: { type: 'string', description: 'File encoding (default: utf-8)' },
      },
      required: ['path'],
    },
  },
  {
    name: 'write_file',
    description: 'Write content to a file',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string' },
        content: { type: 'string' },
        create_dirs: { type: 'boolean', description: 'Create parent directories' },
      },
      required: ['path', 'content'],
    },
  },
  {
    name: 'execute_command',
    description: 'Execute a shell command',
    inputSchema: {
      type: 'object',
      properties: {
        command: { type: 'string' },
        args: { type: 'array', items: { type: 'string' } },
        timeout_ms: { type: 'integer', description: 'Timeout in milliseconds' },
        working_directory: { type: 'string' },
      },
      required: ['command'],
    },
  },
  {
    name: 'search_web',
    description: 'Search the web for information',
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string' },
        max_results: { type: 'integer' },
      },
      required: ['query'],
    },
  },
  {
    name: 'get_status',
    description: 'Get server status (no inputs)',
    // No inputSchema — tools with no parameters
  },
];

// ── Generate the Cedar schema ──

console.log('=== Cedar Schema Generation from MCP Tools ===\n');

const result = generateCedarSchema(tools, {
  namespace: 'MyMcpServer',
  includeTier: true,
  includeAgentId: true,
  includeTimestamp: true,
});

console.log(`Generated schema for ${result.toolCount} tools: ${result.tools.join(', ')}\n`);
console.log('--- .cedarschema (human-readable) ---\n');
console.log(result.schemaText);

console.log('--- Schema JSON (for Cedar WASM) ---\n');
console.log(JSON.stringify(result.schemaJson, null, 2));

// ── Generate a schema stub ──

console.log('\n--- Schema stub (for customization) ---\n');
console.log(generateSchemaStub('MyMcpServer'));

console.log('\n=== Policies enabled by this schema ===\n');
console.log(`With this schema, you can write Cedar policies like:

  // Allow reads only within workspace
  permit(principal, action == Action::"read_file", resource)
  when { context.input.path like "./workspace/*" };

  // Block shell execution entirely
  forbid(principal, action == Action::"execute_command", resource);

  // Allow web search with result limits
  permit(principal, action == Action::"search_web", resource)
  when { context.input.max_results <= 10 };

  // Block writes for untrusted agents
  forbid(principal, action == Action::"write_file", resource)
  when { context.tier == "unknown" };
`);
