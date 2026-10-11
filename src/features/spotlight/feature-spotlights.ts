import type { Spotlight } from './types';

/**
 * Static feature discovery pool. Keep CTAs pointed at real routes/settings only.
 * sinceVersion: first app version where the tip is relevant.
 */
export const FEATURE_SPOTLIGHTS: readonly Spotlight[] = [
  {
    id: 'knowledge-distill-schedule',
    sinceVersion: '0.9.24',
    titleKey: 'spotlight.items.knowledgeDistillSchedule.title',
    bodyKey: 'spotlight.items.knowledgeDistillSchedule.body',
    ctaLabelKey: 'spotlight.items.knowledgeDistillSchedule.cta',
    href: {
      type: 'action',
      action: 'setup-knowledge-distill-schedule',
    },
    surfaces: ['release', 'hub', 'wait'],
    priority: 1,
  },
  {
    id: 'everyday-chrome',
    sinceVersion: '0.9.23',
    titleKey: 'spotlight.items.everydayChrome.title',
    bodyKey: 'spotlight.items.everydayChrome.body',
    ctaLabelKey: 'spotlight.items.everydayChrome.cta',
    href: { type: 'settings', tab: 'system' },
    surfaces: ['release', 'hub', 'wait'],
    priority: 2,
  },
  {
    id: 'starter-tasks',
    sinceVersion: '0.9.10',
    titleKey: 'spotlight.items.starterTasks.title',
    bodyKey: 'spotlight.items.starterTasks.body',
    ctaLabelKey: 'spotlight.items.starterTasks.cta',
    href: { type: 'route', path: '/scheduled-tasks' },
    surfaces: ['release', 'hub', 'wait'],
    priority: 3,
  },
  {
    id: 'visualize-csv',
    sinceVersion: '0.9.8',
    titleKey: 'spotlight.items.visualize.title',
    bodyKey: 'spotlight.items.visualize.body',
    ctaLabelKey: 'spotlight.items.visualize.cta',
    href: { type: 'route', path: '/agent' },
    surfaces: ['release', 'hub'],
    priority: 4,
  },
  {
    id: 'thinking-effort',
    sinceVersion: '0.9.6',
    titleKey: 'spotlight.items.thinkingEffort.title',
    bodyKey: 'spotlight.items.thinkingEffort.body',
    ctaLabelKey: 'spotlight.items.thinkingEffort.cta',
    href: { type: 'settings', tab: 'ai-models' },
    surfaces: ['release', 'hub', 'wait'],
    priority: 5,
  },
  {
    id: 'knowledge-graph',
    sinceVersion: '0.9.5',
    titleKey: 'spotlight.items.knowledgeGraph.title',
    bodyKey: 'spotlight.items.knowledgeGraph.body',
    ctaLabelKey: 'spotlight.items.knowledgeGraph.cta',
    href: { type: 'route', path: '/knowledge' },
    surfaces: ['release', 'hub', 'wait'],
    priority: 6,
  },
  {
    id: 'themes',
    sinceVersion: '0.9.4',
    titleKey: 'spotlight.items.themes.title',
    bodyKey: 'spotlight.items.themes.body',
    ctaLabelKey: 'spotlight.items.themes.cta',
    href: { type: 'settings', tab: 'general' },
    surfaces: ['hub'],
    priority: 7,
  },
];
