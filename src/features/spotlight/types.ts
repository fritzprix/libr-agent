import { z } from 'zod';

export const spotlightHrefSchema = z.discriminatedUnion('type', [
  z.object({
    type: z.literal('settings'),
    tab: z.enum(['general', 'system', 'advanced', 'ai-models']),
  }),
  z.object({
    type: z.literal('route'),
    path: z.string().min(1),
  }),
]);

export type SpotlightHref = z.infer<typeof spotlightHrefSchema>;

export const spotlightSchema = z.object({
  id: z.string().min(1),
  sinceVersion: z.string().min(1),
  titleKey: z.string().min(1),
  bodyKey: z.string().min(1),
  ctaLabelKey: z.string().min(1),
  href: spotlightHrefSchema,
  surfaces: z.array(z.enum(['release', 'hub', 'wait'])).min(1),
  priority: z.number().int(),
});

export type Spotlight = z.infer<typeof spotlightSchema>;

export const spotlightStateSchema = z.object({
  lastSeenAppVersion: z.string(),
  dismissedIds: z.array(z.string()),
  hubHintIndex: z.number().int().nonnegative(),
  waitTipIndex: z.number().int().nonnegative().default(0),
});

export type SpotlightState = z.infer<typeof spotlightStateSchema>;

export const DEFAULT_SPOTLIGHT_STATE: SpotlightState = {
  lastSeenAppVersion: '',
  dismissedIds: [],
  hubHintIndex: 0,
  waitTipIndex: 0,
};

/** Rotate composer busy tip interval (Chat Input strip). */
export const WAIT_TIP_ROTATE_MS = 10_000;

/** @deprecated Prefer WAIT_TIP_ROTATE_MS — kept for older test imports */
export const WAIT_TIP_DELAY_MS = WAIT_TIP_ROTATE_MS;
