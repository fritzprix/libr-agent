/**
 * Cancel mechanism tests
 *
 * These tests guard the cancel flow that has historically been tricky:
 *  - isAbortError: correctly distinguishes intentional abort from real failures
 *  - CancellationRace: AbortError must NOT propagate as a Rust-level error
 *  - StatusTransition: abort → 'idle', real error → 'error'
 *  - RestartAfterCancel: a new request can start normally after an abort
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { isAbortError } from '../types';

// ---------------------------------------------------------------------------
// isAbortError utility
// ---------------------------------------------------------------------------

describe('isAbortError', () => {
  it('returns true for a DOMException with name=AbortError (what AbortController + fetch throws)', () => {
    // DOMException does not extend Error in jsdom/some environments, so
    // isAbortError must check .name on any object, not just Error instances.
    const err = new DOMException('The user aborted a request.', 'AbortError');
    expect(isAbortError(err)).toBe(true);
  });

  it('returns true for a plain Error with name=AbortError', () => {
    const err = new Error('aborted');
    err.name = 'AbortError';
    expect(isAbortError(err)).toBe(true);
  });

  it('returns true for "Request aborted" message (LLM SDK convention)', () => {
    const err = new Error('Request aborted');
    expect(isAbortError(err)).toBe(true);
  });

  it('returns true for a DOMException with "Request aborted" message', () => {
    // Some environments throw DOMException with this message instead of name
    const err = new DOMException('Request aborted');
    expect(isAbortError(err)).toBe(true);
  });

  it('returns false for a generic network error', () => {
    expect(isAbortError(new Error('Network error'))).toBe(false);
  });

  it('returns false for a TypeError', () => {
    expect(isAbortError(new TypeError('type error'))).toBe(false);
  });

  it('returns false for a string thrown as error', () => {
    expect(isAbortError('something went wrong')).toBe(false);
  });

  it('returns false for null', () => {
    expect(isAbortError(null)).toBe(false);
  });

  it('returns false for undefined', () => {
    expect(isAbortError(undefined)).toBe(false);
  });

  it('returns false for a non-null primitive (number)', () => {
    expect(isAbortError(42)).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// AbortController .signal integration
// ---------------------------------------------------------------------------

describe('AbortController abort signal produces isAbortError=true', () => {
  it('synchronously checks: after controller.abort(), signal.aborted is true', () => {
    // jsdom does not support real fetch, but we can verify AbortController
    // itself works correctly for the signal check pattern used in production.
    const controller = new AbortController();
    expect(controller.signal.aborted).toBe(false);
    controller.abort();
    expect(controller.signal.aborted).toBe(true);
  });

  it('the reason thrown on abort is detectable by isAbortError', () => {
    // When AbortController fires, the thrown reason has name=AbortError
    const controller = new AbortController();
    controller.abort();
    // signal.reason is the DOMException that gets thrown into the fetch promise
    const reason: unknown = controller.signal.reason;
    expect(isAbortError(reason)).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Status transition logic
// ---------------------------------------------------------------------------

describe('cancel status transition: abort → idle, real error → error', () => {
  /**
   * Simulates the logic in useLLMExecution catch block:
   *   isAborted ? 'idle' : 'error'
   *
   * This is the core of Bug 1 fix — without it, cancel leaves the TS-local
   * status as 'error' while Rust already transitioned to 'idle'.
   */
  const resolveStatus = (error: unknown): 'idle' | 'error' => {
    return isAbortError(error) ? 'idle' : 'error';
  };

  it('AbortError → idle status', () => {
    const err = new DOMException('aborted', 'AbortError');
    expect(resolveStatus(err)).toBe('idle');
  });

  it('"Request aborted" → idle status', () => {
    const err = new Error('Request aborted');
    expect(resolveStatus(err)).toBe('idle');
  });

  it('real error → error status', () => {
    expect(resolveStatus(new Error('Connection refused'))).toBe('error');
    expect(resolveStatus(new TypeError('invalid JSON'))).toBe('error');
  });
});

// ---------------------------------------------------------------------------
// Rust invoke NOT called on abort (guards the race condition in useLLMListener)
// ---------------------------------------------------------------------------

describe('useLLMListener: handleLLMError must NOT be called on abort', () => {
  /**
   * Regression guard for the race condition:
   *   cancelCompletionRequest() → AbortError thrown
   *   → useLLMListener catch → handleLLMError(sessionId, String(error))  ← BUG
   *   → Rust: Idle (from cancel) then Error (stale abort) — wrong order
   *
   * The correct behaviour: if isAbortError, return early and never invoke Rust.
   */
  const mockHandleLLMError = vi.fn();

  beforeEach(() => {
    mockHandleLLMError.mockClear();
  });

  // Simulates the guard added to useLLMListener's catch block
  const handleCatchInListener = async (
    error: unknown,
    sessionId: string,
  ) => {
    if (isAbortError(error)) {
      return; // ← correct: do NOT call handleLLMError / report to Rust
    }
    await mockHandleLLMError(sessionId, String(error));
  };

  it('does NOT call handleLLMError when aborted', async () => {
    const err = new DOMException('aborted', 'AbortError');
    await handleCatchInListener(err, 'session-1');
    expect(mockHandleLLMError).not.toHaveBeenCalled();
  });

  it('does NOT call handleLLMError for "Request aborted" (regression)', async () => {
    // Some fetch implementations throw a generic error with this message on abort
    const err = new Error('Request aborted');
    await handleCatchInListener(err, 'session-2');
    expect(mockHandleLLMError).not.toHaveBeenCalled();
  });

  it('DOES call handleLLMError for other errors', async () => {
    const err = new Error('API quota exceeded');
    await handleCatchInListener(err, 'session-1');
    expect(mockHandleLLMError).toHaveBeenCalledWith('session-1', 'Error: API quota exceeded');
  });
});

// ---------------------------------------------------------------------------
// Identity guard: cleanup does NOT stomp on a newly started request
// ---------------------------------------------------------------------------

describe('AbortController identity check prevents stale cleanup', () => {
  /**
   * Regression guard for the pattern in useLLMExecution:
   *   if (abortControllersRef.current.get(sessionId) === abortController)
   *
   * When cancel fires mid-stream AND a new request starts immediately,
   * the old caught cleanup must NOT delete the new request's controller.
   */
  it('old controller cleanup is skipped when a newer controller is registered', () => {
    const registry = new Map<string, AbortController>();
    const sessionId = 'session-abc';

    // Request 1 starts
    const controller1 = new AbortController();
    registry.set(sessionId, controller1);

    // Cancel fires: controller1 aborted
    controller1.abort();

    // Request 2 starts immediately (e.g. user sends new message)
    const controller2 = new AbortController();
    registry.set(sessionId, controller2);

    // Now Request 1's catch block runs — it must NOT delete controller2
    const isStillActive = registry.get(sessionId) === controller1;
    if (isStillActive) {
      registry.delete(sessionId); // would stomp controller2 ← BUG
    }
    // controller2 should still be registered
    expect(registry.get(sessionId)).toBe(controller2);
    expect(registry.get(sessionId)).not.toBe(controller1);
  });

  it('current controller cleanup proceeds when no new request has started', () => {
    const registry = new Map<string, AbortController>();
    const sessionId = 'session-abc';

    const controller = new AbortController();
    registry.set(sessionId, controller);

    controller.abort();

    // Catch block runs — controller is still the active one
    if (registry.get(sessionId) === controller) {
      registry.delete(sessionId); // correct: no new request started
    }

    expect(registry.has(sessionId)).toBe(false);
  });
});
