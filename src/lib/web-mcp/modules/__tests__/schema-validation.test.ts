/**
 * Schema validation tests for all Web MCP modules
 * Ensures all tools have valid schemas without empty strings or missing fields
 */

import { describe, it, expect } from 'vitest';
import type { MCPTool } from '@/lib/mcp/protocol/tool';

// Import all Web MCP servers (they are singleton instances)
import bootstrapServer from '../bootstrap-server/server';
import mcpManagerServer from '../mcp-manager/server';
import planningServer from '../planning-server/server';
import playbookStoreServer from '../playbook-store';
import uiToolsServer from '../ui-tools/server';

describe('Web MCP Schema Validation', () => {
  const servers = [
    { name: 'Bootstrap', server: bootstrapServer },
    { name: 'MCP Manager', server: mcpManagerServer },
    { name: 'Planning', server: planningServer },
    { name: 'Playbook Store', server: playbookStoreServer },
    { name: 'UI Tools', server: uiToolsServer },
  ];

  describe('All tools have valid schemas', () => {
    servers.forEach(({ name, server }) => {
      it(`${name} server tools have no empty strings`, () => {
        const tools = server.tools as MCPTool[];
        const errors: string[] = [];

        tools.forEach((tool: MCPTool) => {
          // 1. Tool must have a name
          if (!tool.name || tool.name.trim() === '') {
            errors.push(`${name}: Tool has empty name`);
          }

          // 2. Tool must have a description
          if (!tool.description || tool.description.trim() === '') {
            errors.push(`${name}.${tool.name}: Tool has empty description`);
          }

          // 3. Validate input schema properties
          if (tool.inputSchema?.properties) {
            Object.entries(tool.inputSchema.properties).forEach(([propName, propSchema]) => {
              // Check for empty description
              if ('description' in propSchema && propSchema.description === '') {
                errors.push(`${name}.${tool.name}.${propName}: Property has empty description string`);
              }

              // Check for empty type (type is a string literal, not empty string)
              // Skip this check as JSONSchemaType is a union of string literals

              // Check for undefined description (should be omitted or have value)
              if ('description' in propSchema && propSchema.description === undefined) {
                errors.push(`${name}.${tool.name}.${propName}: Property has undefined description (should be omitted)`);
              }
            });
          }

          // 4. Check for empty required arrays
          if (tool.inputSchema?.required) {
            if (Array.isArray(tool.inputSchema.required) && tool.inputSchema.required.length === 0) {
              errors.push(`${name}.${tool.name}: Has empty required array (should be undefined or omitted)`);
            }
          }
        });

        if (errors.length > 0) {
          console.error(`\n❌ Schema validation errors for ${name}:`);
          errors.forEach(err => console.error(`  - ${err}`));
        }

        expect(errors).toHaveLength(0);
      });

      it(`${name} server tools serialize correctly`, () => {
        const tools = server.tools as MCPTool[];
        const errors: string[] = [];

        tools.forEach((tool: MCPTool) => {
          try {
            const json = JSON.stringify(tool);
            
            // Check for empty string values in JSON
            // Pattern: :"" (but not :"", which is valid in arrays)
            const emptyStringMatches = json.match(/:""\s*[,}]/g);
            if (emptyStringMatches) {
              errors.push(`${name}.${tool.name}: JSON contains empty string values: ${emptyStringMatches.join(', ')}`);
            }

          } catch (error) {
            errors.push(`${name}.${tool.name}: Failed to serialize: ${error}`);
          }
        });

        if (errors.length > 0) {
          console.error(`\n❌ Serialization errors for ${name}:`);
          errors.forEach(err => console.error(`  - ${err}`));
        }

        expect(errors).toHaveLength(0);
      });
    });
  });

  describe('DeepSeek/Ollama compatibility', () => {
    servers.forEach(({ name, server }) => {
      it(`${name} server tools are DeepSeek compatible`, () => {
        const tools = server.tools as MCPTool[];
        const errors: string[] = [];

        tools.forEach((tool: MCPTool) => {
          // DeepSeek/Fireworks specific checks

          // 1. Empty required array check
          if (tool.inputSchema?.required) {
            if (Array.isArray(tool.inputSchema.required) && tool.inputSchema.required.length === 0) {
              errors.push(`${name}.${tool.name}: Empty required array (DeepSeek rejects this)`);
            }
          }

          // 2. Empty string values in schema
          const json = JSON.stringify(tool.inputSchema);
          if (json.includes('""')) {
            const matches = json.match(/:""\s*[,}]/g);
            if (matches && matches.length > 0) {
              errors.push(`${name}.${tool.name}: Contains ${matches.length} empty string value(s)`);
            }
          }
        });

        if (errors.length > 0) {
          console.error(`\n⚠️  DeepSeek compatibility warnings for ${name}:`);
          errors.forEach(err => console.error(`  - ${err}`));
        }

        expect(errors).toHaveLength(0);
      });
    });
  });

  describe('Tool statistics', () => {
    it('should report tool counts per server', () => {
      console.log('\n=== Web MCP Tool Statistics ===');
      
      let totalTools = 0;
      servers.forEach(({ name, server }) => {
        const tools = server.tools as MCPTool[];
        totalTools += tools.length;
        console.log(`${name}: ${tools.length} tools`);
        tools.forEach((tool: MCPTool) => {
          const requiredCount = tool.inputSchema?.required?.length || 0;
          const propsCount = tool.inputSchema?.properties 
            ? Object.keys(tool.inputSchema.properties).length 
            : 0;
          console.log(`  - ${tool.name}: ${propsCount} properties, ${requiredCount} required`);
        });
      });

      console.log(`\nTotal Web MCP tools: ${totalTools}`);
      
      expect(totalTools).toBeGreaterThan(0);
    });
  });
});
