import type { EntityJson } from '@cedar-policy/cedar-wasm'
import type { CedarAgentConfig } from './types.js'

export function generateEntities(config: CedarAgentConfig): EntityJson[] {
  const entities: EntityJson[] = []

  if (config.roles) {
    for (const roleName of Object.keys(config.roles)) {
      entities.push({ uid: { type: 'Role', id: roleName }, attrs: {}, parents: [] })
    }
  }

  const resource = config.resource ?? { type: 'McpServer', id: 'default' }
  entities.push({ uid: resource, attrs: {}, parents: [] })

  return entities
}
