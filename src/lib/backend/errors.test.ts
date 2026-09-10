import { describe, it, expect } from 'vitest';
import {
  DOCKER_NOT_AVAILABLE_PREFIX,
  DOCKER_NOT_INSTALLED_PREFIX,
  classifyDockerAvailabilityError,
  getBackendErrorMessage,
  getDockerNotAvailableMessage,
  isDockerNotAvailableError,
  isDockerNotInstalledError,
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

  describe('isDockerNotInstalledError', () => {
    it('detects structured not-installed prefix', () => {
      expect(
        isDockerNotInstalledError(
          `${DOCKER_NOT_INSTALLED_PREFIX} Docker CLI is not installed or not available on PATH`,
        ),
      ).toBe(true);
    });

    it('detects Display text without prefix', () => {
      expect(
        isDockerNotInstalledError(
          'Docker CLI is not installed or not available on PATH. Details: os error 2',
        ),
      ).toBe(true);
    });

    it('does not treat daemon-offline as not installed', () => {
      expect(
        isDockerNotInstalledError(
          `${DOCKER_NOT_AVAILABLE_PREFIX} Docker is not available`,
        ),
      ).toBe(false);
    });

    it('does not match raw OS not-found strings', () => {
      expect(
        isDockerNotInstalledError(
          'Docker command failed: The system cannot find the file specified. (os error 2)',
        ),
      ).toBe(false);
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

    it('treats not-installed as unavailable so the recovery modal still opens', () => {
      expect(
        isDockerNotAvailableError(
          `${DOCKER_NOT_INSTALLED_PREFIX} Docker CLI is not installed`,
        ),
      ).toBe(true);
    });

    it('returns false for unrelated errors', () => {
      expect(isDockerNotAvailableError('network timeout')).toBe(false);
    });

    it('does not match raw OS not-found strings', () => {
      expect(
        isDockerNotAvailableError(
          'The system cannot find the file specified. (os error 2)',
        ),
      ).toBe(false);
    });
  });

  describe('classifyDockerAvailabilityError', () => {
    it('classifies not-installed before daemon-offline', () => {
      expect(
        classifyDockerAvailabilityError(
          `${DOCKER_NOT_INSTALLED_PREFIX} Docker CLI is not installed`,
        ),
      ).toBe('not-installed');
    });

    it('classifies daemon-offline separately', () => {
      expect(
        classifyDockerAvailabilityError(
          `${DOCKER_NOT_AVAILABLE_PREFIX} Docker is not available`,
        ),
      ).toBe('not-available');
    });

    it('returns null for unrelated errors', () => {
      expect(classifyDockerAvailabilityError('port already in use')).toBeNull();
    });
  });

  describe('getDockerNotAvailableMessage', () => {
    it('strips prefix from structured errors', () => {
      const raw = `${DOCKER_NOT_AVAILABLE_PREFIX} Details here`;
      expect(getDockerNotAvailableMessage(raw)).toBe('Details here');
    });

    it('strips not-installed prefix', () => {
      const raw = `${DOCKER_NOT_INSTALLED_PREFIX} Docker CLI is not installed`;
      expect(getDockerNotAvailableMessage(raw)).toBe(
        'Docker CLI is not installed',
      );
    });
  });
});
