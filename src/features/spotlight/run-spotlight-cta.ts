import { mutate } from 'swr';
import { setupKnowledgeDistillSchedule } from '@/features/scheduled-tasks/setupKnowledgeDistillSchedule';
import { getLogger } from '@/lib/logger';
import { hrefToPath } from './spotlight-state';
import type { Spotlight } from './types';

const logger = getLogger('runSpotlightCta');

export type SpotlightCtaTranslate = (
  key: string,
  options?: { defaultValue?: string },
) => string;

export interface SpotlightCtaResult {
  path: string;
  /** When set, tip should be dismissed after a successful action CTA */
  dismissTipId?: string;
  successMessage?: string;
}

/**
 * Resolve a spotlight CTA: navigate for route/settings, or run a setup action.
 */
export async function runSpotlightCta(
  tip: Spotlight,
  deps: { t: SpotlightCtaTranslate },
): Promise<SpotlightCtaResult> {
  const { t } = deps;
  const { href } = tip;

  if (href.type !== 'action') {
    return { path: hrefToPath(href) };
  }

  if (href.action === 'setup-knowledge-distill-schedule') {
    try {
      const result = await setupKnowledgeDistillSchedule({ t });
      await mutate('scheduled-tasks');
      return {
        path: '/scheduled-tasks',
        dismissTipId: tip.id,
        successMessage: result.created
          ? t('scheduledTasks.setupKnowledgeDistill.created', {
              defaultValue: 'Nightly knowledge distillation task created.',
            })
          : t('scheduledTasks.setupKnowledgeDistill.updated', {
              defaultValue: 'Nightly knowledge distillation task updated.',
            }),
      };
    } catch (error) {
      logger.error('Failed to set up knowledge distill schedule', error);
      throw error;
    }
  }

  const _exhaustive: never = href.action;
  throw new Error(`Unhandled spotlight action: ${_exhaustive}`);
}
