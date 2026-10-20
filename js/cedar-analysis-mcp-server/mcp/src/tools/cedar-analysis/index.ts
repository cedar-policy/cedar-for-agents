import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { registerCedarAnalysisTools } from "./tools.js";

/**
 * Register all Cedar analysis tools with the MCP server
 * 
 * This is the main entry point for the Cedar analysis module.
 * It registers all Cedar-related tools with the provided MCP server instance.
 * 
 * Currently registered tools:
 * - analyze-policies: Analyze a Cedar policy set against a schema to validate policy structure and detect common policy issues such as shadowed permits and logical errors
 * - compare-policy-sets: Compare two Cedar policy sets to verify that policy changes maintain intended authorization behavior
 * 
 * @param server - The MCP server instance to register tools with
 */
export function registerCedarTools(server: McpServer): void {
  registerCedarAnalysisTools(server);
}
