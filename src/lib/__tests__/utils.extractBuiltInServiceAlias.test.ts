import { describe, it, expect } from 'vitest';
import { extractBuiltInServiceAlias, isValidServiceAlias } from '../utils';

describe('extractBuiltInServiceAlias', () => {
  it('should extract simple alias without underscores', () => {
    expect(extractBuiltInServiceAlias('builtin_browser__clickElement')).toBe(
      'browser',
    );
    expect(extractBuiltInServiceAlias('builtin_workspace__readFile')).toBe(
      'workspace',
    );
  });

  it('should extract alias with underscores', () => {
    expect(extractBuiltInServiceAlias('builtin_mcp_manager__list_servers')).toBe(
      'mcp_manager',
    );
    expect(extractBuiltInServiceAlias('builtin_attachments__search')).toBe(
      'attachments',
    );
  });

  it('should extract alias with multiple underscores', () => {
    expect(
      extractBuiltInServiceAlias('builtin_my_long_service_name__doSomething'),
    ).toBe('my_long_service_name');
  });

  it('should handle tool names with underscores', () => {
    expect(
      extractBuiltInServiceAlias('builtin_mcp_manager__search_server'),
    ).toBe('mcp_manager');
    expect(
      extractBuiltInServiceAlias('builtin_browser__execute_async_script'),
    ).toBe('browser');
  });

  it('should return null for invalid patterns', () => {
    expect(extractBuiltInServiceAlias('invalid_tool_name')).toBeNull();
    expect(extractBuiltInServiceAlias('builtin_onlyonepart')).toBeNull();
    expect(extractBuiltInServiceAlias('no_builtin_prefix__tool')).toBeNull();
    expect(extractBuiltInServiceAlias('')).toBeNull();
  });

  it('should use non-greedy matching (stop at first __)', () => {
    // Should match 'service' not 'service__another'
    expect(
      extractBuiltInServiceAlias('builtin_service__another__tool'),
    ).toBe('service');
  });

  describe('edge cases', () => {
    it('should handle service names with many consecutive underscores', () => {
      expect(extractBuiltInServiceAlias('builtin_a_a_a_a__tool')).toBe(
        'a_a_a_a',
      );
      expect(
        extractBuiltInServiceAlias('builtin_a_b_c_d_e_f__tool_name'),
      ).toBe('a_b_c_d_e_f');
    });

    it('should handle service names starting with underscores (edge case)', () => {
      // Note: This is not recommended naming, but the regex handles it
      expect(extractBuiltInServiceAlias('builtin___service__tool')).toBe(
        '__service',
      );
    });

    it('should stop at first __ (important: service names should NOT contain __)', () => {
      // If a service name contains __, only the part before __ is extracted
      // This is why service names MUST NOT contain double underscores
      expect(extractBuiltInServiceAlias('builtin_a__b__tool')).toBe('a');
    });

    it('should handle empty service name (edge case)', () => {
      expect(extractBuiltInServiceAlias('builtin___tool')).toBeNull();
    });

    it('should handle missing tool name', () => {
      expect(extractBuiltInServiceAlias('builtin_service__')).toBe('service');
    });
  });
});

describe('isValidServiceAlias', () => {
  it('should accept valid service names', () => {
    expect(isValidServiceAlias('browser')).toBe(true);
    expect(isValidServiceAlias('mcp_manager')).toBe(true);
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
