// Shared utilities for Web MCP UI modules
// - HTML escaping
// - Creating UIResource objects with attached serviceInfo for routing
// - Handlebars template rendering

import Handlebars from 'handlebars';
import { createUIResource, type UIResource } from '@mcp-ui/server';
import type { ServiceInfo } from '@/lib/mcp-types';

/**
 * Escape HTML special characters to prevent XSS when injecting strings into rawHtml.
 * Note: Handlebars automatically escapes {{variable}} by default.
 * Use {{{variable}}} for unescaped HTML (use with caution).
 */
export function escapeHtml(input: string): string {
  if (!input) return '';
  const map: Record<string, string> = {
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;',
  };
  return String(input).replace(/[&<>"']/g, (c) => map[c]);
}

export interface CreateUiResourceOptions {
  html: string;
  serverName: string; // e.g., 'playbook', 'ui'
  toolName?: string; // optional tool name for routing
  uri?: `ui://${string}`; // optional explicit URI; if omitted, a default will be generated
}

/**
 * Create a UIResource with consistent serviceInfo for proper frontend routing.
 */
export function createUiResourceWithServiceInfo(
  opts: CreateUiResourceOptions,
): UIResource & { serviceInfo?: ServiceInfo } {
  const { html, serverName, toolName = '', uri } = opts;

  const res = createUIResource({
    uri: uri ?? (`ui://${serverName}/${Date.now()}` as `ui://${string}`),
    content: { type: 'rawHtml', htmlString: html },
    encoding: 'text',
  }) as UIResource & { serviceInfo?: ServiceInfo };

  res.serviceInfo = {
    serverName,
    toolName,
    backendType: 'BuiltInWeb',
  };

  return res;
}

/**
 * Render a Handlebars template with the provided data.
 * Handlebars automatically escapes {{variable}} by default for XSS protection.
 * Use {{{variable}}} (triple braces) for unescaped HTML when needed.
 *
 * @param templateString - The Handlebars template string
 * @param data - Data object to pass to the template
 * @returns Rendered HTML string
 */
export function renderTemplate(
  templateString: string,
  data: Record<string, unknown>,
): string {
  const template = Handlebars.compile(templateString);
  return template(data);
}
