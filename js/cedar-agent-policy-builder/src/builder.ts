import type { BuildResult, CedarAgentConfig, McpToolDefinition, PrincipalConfig, SchemaConfig } from './types.js'
import { generatePolicies } from './policy-generators.js'
import { generateEntities } from './entities.js'
import { generateSchema } from './schema.js'

export function fromConfig(config: CedarAgentConfig): BuildResult {
  const result: BuildResult = {
    policies: generatePolicies(config),
    entities: generateEntities(config),
  }
  if (config.tools) {
    result.schema = generateSchema(config)
  }
  return result
}

export class CedarAgentPolicyBuilder {
  private _config: CedarAgentConfig

  constructor(schema?: SchemaConfig) {
    this._config = {
      principal: schema?.principal ?? { key: 'user_id' },
      resource: schema?.resource,
      tools: schema?.tools,
      namespace: schema?.namespace,
    }
  }

  role(name: string, tools: string[]): this {
    this._config.roles ??= {}
    this._config.roles[name] = tools
    return this
  }

  restrict(tool: string, config: { allowedValues: Record<string, unknown[]> }): this {
    this._config.restrictions ??= {}
    this._config.restrictions[tool] = config
    return this
  }

  // Note: maxPerSession of 0 means the tool is effectively denied (call_count >= 0 is always true).
  rateLimit(tool: string, maxPerSession: number): this {
    this._config.rateLimits ??= {}
    this._config.rateLimits[tool] = maxPerSession
    return this
  }

  // Note: hourStart == hourEnd produces a zero-width window that denies at all hours.
  timeWindow(config: { hourStart: number; hourEnd: number }): this {
    this._config.timeWindow = config
    return this
  }

  denyToolsInEnv(env: string, tools?: string[]): this {
    this._config.denyInEnv ??= {}
    this._config.denyInEnv[env] = tools ?? ['*']
    return this
  }

  /** Require human consent before executing these tools.
   * Without forRole: all roles need consent (overwrites any prior role-specific consent for these tools).
   * With forRole: only that role needs consent (accumulates with other role-specific calls). */
  consent(tools: string[], forRole?: string): this {
    this._config.consent ??= {}
    for (const tool of tools) {
      this._config.consent[tool] ??= []
      if (forRole) {
        this._config.consent[tool].push(forRole)
      } else {
        this._config.consent[tool] = ['*']
      }
    }
    return this
  }

  build(): BuildResult {
    this._warnUnknownTools()
    return fromConfig(this._config)
  }

  private _warnUnknownTools(): void {
    if (!this._config.roles) return
    const declaredTools = new Set(
      Object.values(this._config.roles).flat().filter((t) => t !== '*')
    )
    if (declaredTools.size === 0) return

    const referenced = new Set<string>()
    if (this._config.restrictions) {
      for (const tool of Object.keys(this._config.restrictions)) referenced.add(tool)
    }
    if (this._config.rateLimits) {
      for (const tool of Object.keys(this._config.rateLimits)) referenced.add(tool)
    }
    if (this._config.denyInEnv) {
      for (const tools of Object.values(this._config.denyInEnv)) {
        for (const tool of tools) if (tool !== '*') referenced.add(tool)
      }
    }
    if (this._config.consent) {
      for (const tool of Object.keys(this._config.consent)) referenced.add(tool)
    }

    for (const tool of referenced) {
      if (!declaredTools.has(tool)) {
        console.warn(`[cedar-agent-policy-builder] Warning: "${tool}" is referenced in restrict/denyToolsInEnv/consent but not declared in any role`)
      }
    }
  }
}
