import { describe, expect, it } from 'vitest';
import type { ServerMetadata } from '@/lib/mcp/config/server-config';
import {
  getTransportEndpointLabel,
  sanitizeHttpDisplayUrl,
} from '../transport-display';

describe('sanitizeHttpDisplayUrl', () => {
  it('returns the original string when the URL is invalid', () => {
    expect(sanitizeHttpDisplayUrl('not-a-url')).toBe('not-a-url');
  });

  it('strips url-param variables defined in metadata', () => {
    const metadata: ServerMetadata = {
      variableDefinitions: {
        tools: { target: 'url-param' },
        api_key: { target: 'url-param' },
      },
    };

    expect(
      sanitizeHttpDisplayUrl(
        'https://mcp.exa.ai/mcp?tools=web_search_advanced_exa%2Cweb_search_exa&api_key=secret',
        metadata,
      ),
    ).toBe('https://mcp.exa.ai/mcp');
  });

  it('keeps query params that are not url-param variables', () => {
    const metadata: ServerMetadata = {
      variableDefinitions: {
        api_key: { target: 'url-param' },
      },
    };

    expect(
      sanitizeHttpDisplayUrl(
        'https://mcp.exa.ai/mcp?tools=web_search_exa&api_key=secret',
        metadata,
      ),
    ).toBe('https://mcp.exa.ai/mcp?tools=web_search_exa');
  });
});

describe('getTransportEndpointLabel', () => {
  it('returns the stdio command', () => {
    expect(
      getTransportEndpointLabel({
        type: 'stdio',
        command: 'npx',
        args: ['-y', 'exa-mcp'],
      }),
    ).toBe('npx');
  });

  it('returns the sanitized http-sse URL', () => {
    expect(
      getTransportEndpointLabel(
        {
          type: 'http-sse',
          url: 'https://mcp.exa.ai/mcp?api_key=secret',
        },
        {
          variableDefinitions: {
            api_key: { target: 'url-param' },
          },
        },
      ),
    ).toBe('https://mcp.exa.ai/mcp');
  });
});
