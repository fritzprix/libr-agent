import { listAssistants } from '@/lib/backend/assistants';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import type { NavigateFunction } from 'react-router-dom';

const logger = getLogger('openAppWizard');

export const APP_WIZARD_ASSISTANT_NAME = 'App Wizard';

/** DefaultValue for `onboarding.appWizardPrompt` i18n key. */
export const APP_WIZARD_ONBOARDING_PROMPT =
  'Check whether Node.js (npx) and uv are installed on this machine. If missing, install them and tell me that an app restart may be required for PATH updates.';

export interface NavigateToAppWizardOptions {
  prompt?: string;
  autoSubmit?: boolean;
  /** Called with a translation-aware message helper if provided by the caller. */
  t?: (key: string, defaultValue: string) => string;
}

/**
 * Opens the built-in App Wizard assistant in a draft session.
 * Falls back to the Chat hub with a toast if the assistant cannot be resolved.
 * Errors are toasted and then rethrown so callers can stop loading state safely.
 */
export async function navigateToAppWizard(
  navigate: NavigateFunction,
  options: NavigateToAppWizardOptions = {},
): Promise<void> {
  const {
    prompt = APP_WIZARD_ONBOARDING_PROMPT,
    autoSubmit = true,
    t = (_key, defaultValue) => defaultValue,
  } = options;

  try {
    const assistants = await listAssistants();
    const wizard = assistants.find(
      (assistant) => assistant.name === APP_WIZARD_ASSISTANT_NAME,
    );
    if (wizard) {
      const params = new URLSearchParams({
        assistantId: wizard.id,
        prompt,
        autoSubmit: autoSubmit ? 'true' : 'false',
      });
      navigate(`/agent/draft?${params.toString()}`);
      return;
    }

    navigate('/agent');
    toast.message(
      t(
        'onboarding.appWizardFallback',
        'Open App Wizard from Chat and ask it to install Node.js / uv.',
      ),
    );
  } catch (error) {
    logger.error('Failed to open App Wizard', error);
    navigate('/agent');
    toast.error(
      t(
        'onboarding.appWizardOpenFailed',
        'Could not open App Wizard. Go to Chat and select App Wizard.',
      ),
    );
    throw error;
  }
}
