// #2923 (gatekeeper G1.3d): the Assistant card shows the plugin read-only
// badge and hides Edit for plugin-owned assistants.
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import AssistantCard from '../Card';
import type { Assistant } from '@/models/chat';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: string | { defaultValue?: string }) => {
      if (typeof options === 'string') return options;
      if (options && typeof options === 'object' && 'defaultValue' in options) {
        return options.defaultValue ?? key;
      }
      return key;
    },
  }),
}));

vi.mock('@/context/AssistantContext', () => ({
  useAssistantContext: () => ({ saveAssistant: vi.fn() }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('sonner', () => ({
  toast: { error: vi.fn(), success: vi.fn(), info: vi.fn() },
}));

vi.mock('../AssistantEditor', () => ({
  default: { Dialog: () => null },
}));

function assistantWith(overrides: Partial<Assistant>): Assistant {
  return {
    id: 'assistant-1',
    name: 'Reviewer',
    systemPrompt: 'You review code.',
    description: 'Reviews code',
    deletionProtected: false,
    createdAt: new Date(0),
    updatedAt: new Date(0),
    ...overrides,
  };
}

function renderCard(assistant: Assistant) {
  return render(
    <AssistantCard
      assistant={assistant}
      isExpanded={false}
      onToggle={vi.fn()}
      onDelete={vi.fn()}
    />,
  );
}

describe('AssistantCard plugin ownership (#2923)', () => {
  it('shows the plugin badge and hides Edit for plugin-owned assistants', () => {
    renderCard(
      assistantWith({
        id: 'plugin:plugin-aa:reviewer',
        pluginSource: 'plugin:plugin-aa',
        pluginSourceName: 'myp',
        deletionProtected: true,
      }),
    );

    expect(screen.getByText(/Plugin: /)).toBeInTheDocument();
    expect(screen.queryByText('assistant.card.edit')).not.toBeInTheDocument();
  });

  it('keeps Edit available for normal assistants', () => {
    renderCard(assistantWith({}));

    expect(screen.queryByText(/Plugin: /)).not.toBeInTheDocument();
    expect(screen.getByText('assistant.card.edit')).toBeInTheDocument();
  });
});
