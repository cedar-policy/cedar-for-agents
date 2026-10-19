import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { registerAllTools } from "./tools/index.js";
import { registerAllPrompts } from "./prompts.js";

/**
 * Cedar Analysis MCP Server
 * 
 * This server provides tools for analyzing Cedar authorization policies:
 * - analyze-policies: Analyze a Cedar policy set against a schema to validate policy structure and detect common policy issues such as shadowed permits and logical errors
 * - compare-policy-sets: Compare two Cedar policy sets to verify that policy changes maintain intended authorization behavior
 * 
 * Prompts:
 * - add-and-verify-new-policy-workflow: Analyzes the impact of adding new Cedar policies to your existing policy set
 */

// Create server instance
const server = new McpServer({
  name: "cedar-analysis",
  version: "1.0.0",
});

// Register all tools
registerAllTools(server);

// Register all prompts
registerAllPrompts(server);

/**
 * Main function to run the server
 */
async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

// Run the server
main().catch((error) => {
  process.exit(1);
});
