import type { EntityJson, TypeAndId, CedarValueJson } from '@cedar-policy/cedar-wasm'

export type { EntityJson, TypeAndId, CedarValueJson }

export interface PrincipalConfig {
  key: string
  type?: string
}

export interface McpToolDefinition {
  name: string
  description?: string
  inputSchema: Record<string, unknown>
  outputSchema?: Record<string, unknown>
}

export interface SchemaConfig {
  principal?: PrincipalConfig
  resource?: { type: string; id: string }
  tools?: McpToolDefinition[]
  namespace?: string
}

export interface CedarAgentConfig {
  principal: PrincipalConfig
  roles?: Record<string, string[]>
  restrictions?: Record<string, {
    allowedValues: Record<string, unknown[]>
  }>
  rateLimits?: Record<string, number>
  timeWindow?: { hourStart: number; hourEnd: number }
  denyInEnv?: Record<string, string[]>
  consent?: Record<string, string[]>
  resource?: { type: string; id: string }
  tools?: McpToolDefinition[]
  namespace?: string
}

export interface BuildResult {
  policies: string
  entities: EntityJson[]
  schema?: string
}
