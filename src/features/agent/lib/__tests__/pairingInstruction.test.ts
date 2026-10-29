import { describe, expect, it } from 'vitest';
import { getAgentIntegrationInstruction } from '../pairingInstruction';

describe('pairingInstruction', () => {
  it('generates markdown instruction containing necessary steps and fallback port', () => {
    const instruction = getAgentIntegrationInstruction(3030);
    expect(instruction).toContain('/api/pairings');
    expect(instruction).toContain('xxx-yyy');
    expect(instruction).toContain('http_port');
    expect(instruction).toContain('3030');
    expect(instruction).toContain('Authorization: Bearer <apiKey>');
  });

  it('customizes fallback port when provided', () => {
    const instruction = getAgentIntegrationInstruction(4040);
    expect(instruction).toContain('4040');
  });
});
