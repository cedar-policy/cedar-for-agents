import type { CedarAgentConfig } from './types.js'
import { generateSchema as generateSchemaWasm } from '@cedar-policy/mcp-schema-generator-wasm'

function buildSchemaStub(config: CedarAgentConfig): string {
  const ns = config.namespace ?? 'Agent'
  const principalType = config.principal.type ?? 'User'
  const resourceType = config.resource?.type ?? 'McpServer'

  return `namespace ${ns} {
  entity Role;

  @mcp_principal("${principalType}")
  entity ${principalType} in [Role];

  @mcp_resource("${resourceType}")
  entity ${resourceType};
}`
}

function buildToolsJson(config: CedarAgentConfig): string {
  const tools = (config.tools ?? []).map((t) => ({
    name: t.name,
    description: t.description ?? '',
    inputSchema: t.inputSchema,
    ...(t.outputSchema ? { outputSchema: t.outputSchema } : {}),
  }))
  return JSON.stringify({ result: { tools } })
}

export function generateSchema(config: CedarAgentConfig): string | undefined {
  if (!config.tools || config.tools.length === 0) return undefined

  const schemaStub = buildSchemaStub(config)
  const toolsJson = buildToolsJson(config)
  const raw = generateSchemaWasm(schemaStub, toolsJson, '{}')
  const result = typeof raw === 'string' ? JSON.parse(raw) : raw

  if (!result.isOk) {
    throw new Error(`Schema generation failed: ${result.error}`)
  }

  return result.schema
}
