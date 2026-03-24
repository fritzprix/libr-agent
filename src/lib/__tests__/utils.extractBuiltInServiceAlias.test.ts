import { describe, it, expect } from 'vitest';
import { extractBuiltInServiceAlias, isValidServiceAlias } from '../utils';

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
    expect(extractBuiltInServiceAlias('my_long_service_name__doSomething')).toBeNull();
    expect(extractBuiltInServiceAlias('external_server__tool')).toBeNull();
  });

  it('should return null for invalid patterns', () => {
    expect(extractBuiltInServiceAlias('invalid_tool_name')).toBeNull();
    expect(extractBuiltInServiceAlias('')).toBeNull();
    expect(extractBuiltInServiceAlias('no_double_underscore')).toBeNull();
  });

  it('should stop at first __ (important: service names should NOT contain __)', () => {
    expect(extractBuiltInServiceAlias('browser__another__tool')).toBe('browser');
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
