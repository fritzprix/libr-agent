import { describe, it, expect } from 'vitest';
import {
  hasServiceInfo,
  extractServiceInfoFromContent,
} from '../service-info';
import { MCPContent, ServiceInfo } from '../../protocol';

describe('MCP Service Info Utils', () => {
  const mockServiceInfo: ServiceInfo = {
    serverName: 'TestServer',
    toolName: 'TestTool',
    backendType: 'ExternalMCP',
  };

  const mockContentWithServiceInfo: MCPContent = {
    type: 'text',
    text: 'Hello',
    serviceInfo: mockServiceInfo,
  };

  const mockContentWithoutServiceInfo: MCPContent = {
    type: 'text',
    text: 'Hello',
  };

  describe('hasServiceInfo', () => {
    it('should return true if content has serviceInfo', () => {
      expect(hasServiceInfo(mockContentWithServiceInfo)).toBe(true);
    });

    it('should return false if content does not have serviceInfo', () => {
      expect(hasServiceInfo(mockContentWithoutServiceInfo)).toBe(false);
    });

    it('should return false if content is null/undefined', () => {
        // @ts-expect-error - Testing invalid input
      expect(hasServiceInfo(null)).toBe(false);
        // @ts-expect-error - Testing invalid input
      expect(hasServiceInfo(undefined)).toBe(false);
    });
  });

  describe('extractServiceInfoFromContent', () => {
    it('should extract service info from content array', () => {
      const content = [
        mockContentWithoutServiceInfo,
        mockContentWithServiceInfo,
      ];
      expect(extractServiceInfoFromContent(content)).toEqual(mockServiceInfo);
    });

    it('should return null if no service info found', () => {
      const content = [
        mockContentWithoutServiceInfo,
      ];
      expect(extractServiceInfoFromContent(content)).toBeNull();
    });

    it('should return null for empty array', () => {
      expect(extractServiceInfoFromContent([])).toBeNull();
    });
  });
});
