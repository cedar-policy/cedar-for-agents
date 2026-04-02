/**
 * Tests for Cedar schema generation from MCP tool descriptions.
 *
 * Run: node test.mjs
 */

import { generateCedarSchema, generateSchemaStub } from 'protect-mcp';
import assert from 'node:assert';

let passed = 0;
let failed = 0;

function test(name, fn) {
  try {
    fn();
    console.log(`  PASS  ${name}`);
    passed++;
  } catch (err) {
    console.log(`  FAIL  ${name}: ${err.message}`);
    failed++;
  }
}

console.log('\nCedar Schema Generation Tests\n');

// ── Sample tools ──

const tools = [
  {
    name: 'read_file',
    inputSchema: {
      type: 'object',
      properties: { path: { type: 'string' } },
      required: ['path'],
    },
  },
  {
    name: 'execute_command',
    inputSchema: {
      type: 'object',
      properties: {
        command: { type: 'string' },
        args: { type: 'array', items: { type: 'string' } },
        timeout: { type: 'integer' },
      },
      required: ['command'],
    },
  },
  { name: 'get_status' }, // No input schema
];

// ── Tests ──

test('generates schema with correct tool count', () => {
  const result = generateCedarSchema(tools);
  assert.strictEqual(result.toolCount, 3);
});

test('includes all tool names', () => {
  const result = generateCedarSchema(tools);
  assert.deepStrictEqual(result.tools, ['read_file', 'execute_command', 'get_status']);
});

test('schema text contains namespace', () => {
  const result = generateCedarSchema(tools, { namespace: 'TestNS' });
  assert.ok(result.schemaText.includes('namespace TestNS {'));
});

test('schema text contains per-tool actions', () => {
  const result = generateCedarSchema(tools);
  assert.ok(result.schemaText.includes('action "read_file"'));
  assert.ok(result.schemaText.includes('action "execute_command"'));
  assert.ok(result.schemaText.includes('action "get_status"'));
});

test('schema text contains blanket action', () => {
  const result = generateCedarSchema(tools);
  assert.ok(result.schemaText.includes('action "MCP::Tool::call"'));
});

test('maps string to String', () => {
  const result = generateCedarSchema(tools);
  assert.ok(result.schemaText.includes('"path": String'));
});

test('maps integer to Long', () => {
  const result = generateCedarSchema(tools);
  assert.ok(result.schemaText.includes('"timeout": Long'));
});

test('maps array to Set', () => {
  const result = generateCedarSchema(tools);
  assert.ok(result.schemaText.includes('Set<String>'));
});

test('includes Agent and Tool entities', () => {
  const result = generateCedarSchema(tools);
  assert.ok(result.schemaText.includes('entity Agent'));
  assert.ok(result.schemaText.includes('entity Tool;'));
});

test('schema JSON has correct structure', () => {
  const result = generateCedarSchema(tools);
  assert.ok(result.schemaJson['ScopeBlind']);
  const ns = result.schemaJson['ScopeBlind'];
  assert.ok(ns.entityTypes);
  assert.ok(ns.actions);
});

test('schema JSON contains all actions', () => {
  const result = generateCedarSchema(tools);
  const ns = result.schemaJson['ScopeBlind'];
  assert.ok(ns.actions['read_file']);
  assert.ok(ns.actions['execute_command']);
  assert.ok(ns.actions['get_status']);
  assert.ok(ns.actions['MCP::Tool::call']);
});

test('handles tools with no input schema', () => {
  const result = generateCedarSchema([{ name: 'ping' }]);
  assert.strictEqual(result.toolCount, 1);
  assert.ok(result.schemaText.includes('action "ping"'));
});

test('generateSchemaStub produces valid stub', () => {
  const stub = generateSchemaStub('TestNS');
  assert.ok(stub.includes('namespace TestNS {'));
  assert.ok(stub.includes('entity Agent'));
  assert.ok(stub.includes('entity Tool'));
  assert.ok(stub.includes('cedar-for-agents'));
});

// ── Summary ──

console.log(`\n${passed} passed, ${failed} failed\n`);
if (failed > 0) process.exit(1);
