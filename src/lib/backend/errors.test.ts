import { describe, it, expect } from 'vitest';
import {
  DOCKER_NOT_AVAILABLE_PREFIX,
  getBackendErrorMessage,
  getDockerNotAvailableMessage,
  isDockerNotAvailableError,
  stripErrorCodePrefix,
} from './errors';

describe('backend/errors', () => {
  describe('getBackendErrorMessage', () => {
    it('returns string errors as-is', () => {
      expect(getBackendErrorMessage('plain error')).toBe('plain error');
    });

    it('returns Error.message', () => {
      expect(getBackendErrorMessage(new Error('boom'))).toBe('boom');
    });

    it('returns message field from object-shaped errors', () => {
      expect(getBackendErrorMessage({ message: 'from object' })).toBe(
        'from object',
      );
    });
  });

  describe('stripErrorCodePrefix', () => {
    it('strips known prefix', () => {
      const raw = `${DOCKER_NOT_AVAILABLE_PREFIX} Docker is not available`;
      expect(stripErrorCodePrefix(raw, DOCKER_NOT_AVAILABLE_PREFIX)).toBe(
        'Docker is not available',
      );
    });

    it('leaves messages without prefix unchanged', () => {
      expect(stripErrorCodePrefix('other', DOCKER_NOT_AVAILABLE_PREFIX)).toBe(
        'other',
      );
    });
  });

  describe('isDockerNotAvailableError', () => {
    it('detects structured prefix', () => {
      expect(
        isDockerNotAvailableError(
          `${DOCKER_NOT_AVAILABLE_PREFIX} Docker is not available`,
        ),
      ).toBe(true);
    });

    it('detects legacy message text', () => {
      expect(
        isDockerNotAvailableError('Docker is not available. Ensure Docker...'),
      ).toBe(true);
    });

    it('returns false for unrelated errors', () => {
      expect(isDockerNotAvailableError('network timeout')).toBe(false);
    });
  });

  describe('getDockerNotAvailableMessage', () => {
    it('strips prefix from structured errors', () => {
      const raw = `${DOCKER_NOT_AVAILABLE_PREFIX} Details here`;
      expect(getDockerNotAvailableMessage(raw)).toBe('Details here');
    });
  });
});
