import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Spotlight } from '../types';
import { runSpotlightCta } from '../run-spotlight-cta';

const setupKnowledgeDistillSchedule = vi.fn();
const mutate = vi.fn();

vi.mock('@/features/scheduled-tasks/setupKnowledgeDistillSchedule', () => ({
  setupKnowledgeDistillSchedule: (...args: unknown[]) =>
    setupKnowledgeDistillSchedule(...args),
}));

vi.mock('swr', () => ({
  mutate: (...args: unknown[]) => mutate(...args),
}));

const t = (key: string, options?: { defaultValue?: string }) =>
  options?.defaultValue ?? key;

describe('runSpotlightCta', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns a navigate path for route hrefs', async () => {
    const tip: Spotlight = {
      id: 'starter-tasks',
      sinceVersion: '0.9.10',
      titleKey: 't',
      bodyKey: 'b',
      ctaLabelKey: 'c',
      href: { type: 'route', path: '/scheduled-tasks' },
      surfaces: ['hub'],
      priority: 1,
    };

    await expect(runSpotlightCta(tip, { t })).resolves.toEqual({
      path: '/scheduled-tasks',
    });
    expect(setupKnowledgeDistillSchedule).not.toHaveBeenCalled();
  });

  it('runs knowledge distill setup for action hrefs', async () => {
    setupKnowledgeDistillSchedule.mockResolvedValue({
      created: true,
      assistantId: 'ast-1',
      task: { id: 'task-1' },
    });

    const tip: Spotlight = {
      id: 'knowledge-distill-schedule',
      sinceVersion: '0.9.24',
      titleKey: 't',
      bodyKey: 'b',
      ctaLabelKey: 'c',
      href: {
        type: 'action',
        action: 'setup-knowledge-distill-schedule',
      },
      surfaces: ['hub', 'wait'],
      priority: 1,
    };

    const result = await runSpotlightCta(tip, { t });

    expect(setupKnowledgeDistillSchedule).toHaveBeenCalled();
    expect(mutate).toHaveBeenCalledWith('scheduled-tasks');
    expect(result).toEqual({
      path: '/scheduled-tasks',
      dismissTipId: 'knowledge-distill-schedule',
      successMessage: 'Nightly knowledge distillation task created.',
    });
  });
});
