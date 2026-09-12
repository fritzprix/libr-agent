import { describe, it, expect } from 'vitest';
import {
  extractBuiltInServiceAlias,
  isValidMcpServerName,
  isValidServiceAlias,
  sanitizeMcpServerName,
} from '../utils';

describe('extractBuiltInServiceAlias', () => {
  it('should extract simple alias for known builtin services', () => {
    expect(extractBuiltInServiceAlias('browser__clickElement')).toBe('browser');
    expect(extractBuiltInServiceAlias('workspace__readFile')).toBe('workspace');
    expect(extractBuiltInServiceAlias('planning__addTodo')).toBe('planning');
  });

  it('should extract alias with underscores for known services', () => {
    expect(extractBuiltInServiceAlias('attachments__search')).toBe(
      'attachments',
    );
  });

  it('should return null for unknown services', () => {
    expect(
      extractBuiltInServiceAlias('my_long_service_name__doSomething'),
    ).toBeNull();
    expect(extractBuiltInServiceAlias('external_server__tool')).toBeNull();
  });

  it('should return null for invalid patterns', () => {
    expect(extractBuiltInServiceAlias('invalid_tool_name')).toBeNull();
    expect(extractBuiltInServiceAlias('')).toBeNull();
    expect(extractBuiltInServiceAlias('no_double_underscore')).toBeNull();
  });

  it('should stop at first __ (important: service names should NOT contain __)', () => {
    expect(extractBuiltInServiceAlias('browser__another__tool')).toBe(
      'browser',
    );
  });

  describe('edge cases', () => {
    it('should return null for missing tool name after __', () => {
      expect(extractBuiltInServiceAlias('browser__')).toBeNull();
    });

    it('should return null if server part is empty', () => {
      expect(extractBuiltInServiceAlias('__tool')).toBeNull();
    });
  });
});

describe('sanitizeMcpServerName', () => {
  it('replaces spaces and collapses runs', () => {
    expect(sanitizeMcpServerName('My Server')).toBe('My_Server');
    expect(sanitizeMcpServerName('My  Server')).toBe('My_Server');
    expect(sanitizeMcpServerName('Google Drive')).toBe('Google_Drive');
  });

  it('replaces hyphens and punctuation', () => {
    expect(sanitizeMcpServerName('yahoo-finance')).toBe('yahoo_finance');
    expect(sanitizeMcpServerName('a.b@c')).toBe('a_b_c');
  });

  it('prefixes digit-leading names', () => {
    expect(sanitizeMcpServerName('2FA Server')).toBe('s_2FA_Server');
  });

  it('falls back for empty or symbol-only input', () => {
    expect(sanitizeMcpServerName('')).toBe('mcp_server');
    expect(sanitizeMcpServerName('@@@')).toBe('mcp_server');
  });

  it('never emits double underscores', () => {
    for (const sample of ['My  Server', 'a--b', 'a__b', ' x - y ']) {
      const sanitized = sanitizeMcpServerName(sample);
      expect(sanitized.includes('__')).toBe(false);
      expect(isValidMcpServerName(sanitized)).toBe(true);
    }
  });
});

describe('isValidMcpServerName', () => {
  it('accepts Gemini-safe identifiers', () => {
    expect(isValidMcpServerName('filesystem')).toBe(true);
    expect(isValidMcpServerName('My_Server')).toBe(true);
    expect(isValidMcpServerName('_x')).toBe(true);
  });

  it('rejects spaces, hyphens, digits-first, and __', () => {
    expect(isValidMcpServerName('My Server')).toBe(false);
    expect(isValidMcpServerName('yahoo-finance')).toBe(false);
    expect(isValidMcpServerName('2bad')).toBe(false);
    expect(isValidMcpServerName('a__b')).toBe(false);
  });
});

describe('isValidServiceAlias', () => {
  it('should accept valid service names', () => {
    expect(isValidServiceAlias('browser')).toBe(true);
    expect(isValidServiceAlias('tool')).toBe(true);
    expect(isValidServiceAlias('attachments')).toBe(true);
    expect(isValidServiceAlias('a_b_c_d_e_f')).toBe(true);
    expect(isValidServiceAlias('my_service_123')).toBe(true);
  });

  it('should reject empty or whitespace names', () => {
    expect(isValidServiceAlias('')).toBe(false);
    expect(isValidServiceAlias('   ')).toBe(false);
  });

  it('should reject names with double underscores', () => {
    expect(isValidServiceAlias('a__b')).toBe(false);
    expect(isValidServiceAlias('service__name')).toBe(false);
    expect(isValidServiceAlias('__service')).toBe(false);
    expect(isValidServiceAlias('service__')).toBe(false);
  });

  it('should reject names with invalid characters', () => {
    expect(isValidServiceAlias('service-name')).toBe(false);
    expect(isValidServiceAlias('service.name')).toBe(false);
    expect(isValidServiceAlias('service name')).toBe(false);
    expect(isValidServiceAlias('service@name')).toBe(false);
  });

  it('should reject names starting or ending with underscore', () => {
    expect(isValidServiceAlias('_service')).toBe(false);
    expect(isValidServiceAlias('service_')).toBe(false);
  });

  it('should reject names with consecutive underscores', () => {
    expect(isValidServiceAlias('a___b')).toBe(false);
    expect(isValidServiceAlias('service___name')).toBe(false);
  });
});
