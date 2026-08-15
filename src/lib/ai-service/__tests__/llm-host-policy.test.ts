import { describe, expect, it } from 'vitest';
import {
  isSelfHostedLlmBaseUrl,
  resolveSelfHostedLlmClientOptions,
  SELF_HOSTED_LLM_MAX_RETRIES,
  SELF_HOSTED_LLM_TIMEOUT_MS,
} from '../llm-host-policy';

describe('llm-host-policy', () => {
  it('detects loopback and private hosts as self-hosted', () => {
    expect(isSelfHostedLlmBaseUrl('http://127.0.0.1:8080/v1')).toBe(true);
    expect(isSelfHostedLlmBaseUrl('http://localhost:1234/v1')).toBe(true);
    expect(isSelfHostedLlmBaseUrl('http://10.0.0.5:8000/v1')).toBe(true);
    expect(isSelfHostedLlmBaseUrl('http://192.168.1.10:8080/v1')).toBe(true);
    expect(isSelfHostedLlmBaseUrl('http://172.16.0.2:8080/v1')).toBe(true);
    expect(isSelfHostedLlmBaseUrl('http://llama.local:8080/v1')).toBe(true);
    expect(isSelfHostedLlmBaseUrl('http://host.docker.internal:8080/v1')).toBe(
      true,
    );
    expect(isSelfHostedLlmBaseUrl('127.0.0.1:11434')).toBe(true);
    expect(isSelfHostedLlmBaseUrl('localhost:8080/v1')).toBe(true);
  });

  it('treats cloud and empty URLs as not self-hosted', () => {
    expect(isSelfHostedLlmBaseUrl(undefined)).toBe(false);
    expect(isSelfHostedLlmBaseUrl('')).toBe(false);
    expect(isSelfHostedLlmBaseUrl('https://api.openai.com/v1')).toBe(false);
    expect(isSelfHostedLlmBaseUrl('https://openrouter.ai/api/v1')).toBe(false);
    expect(isSelfHostedLlmBaseUrl('not a url')).toBe(false);
  });

  it('resolves 15-minute timeout and zero retries for self-hosted hosts', () => {
    expect(
      resolveSelfHostedLlmClientOptions('http://127.0.0.1:8080/v1'),
    ).toEqual({
      timeout: SELF_HOSTED_LLM_TIMEOUT_MS,
      maxRetries: SELF_HOSTED_LLM_MAX_RETRIES,
    });

    expect(
      resolveSelfHostedLlmClientOptions('http://127.0.0.1:8080/v1', {
        timeout: 1_200_000,
        maxRetries: 3,
      }),
    ).toEqual({
      timeout: 1_200_000,
      maxRetries: 0,
    });

    expect(
      resolveSelfHostedLlmClientOptions('https://api.openai.com/v1'),
    ).toBeUndefined();
  });
});
