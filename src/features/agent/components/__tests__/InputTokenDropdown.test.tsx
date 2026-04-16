import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { describe, expect, it, vi } from 'vitest';
import type { SkillMetadata } from '@/types/skills';
import { InputTokenDropdown } from '../InputTokenDropdown';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
  }),
}));

describe('InputTokenDropdown', () => {
  it('renders skill source badges and inserts the selected skill', () => {
    const onSelectArg = vi.fn();
    const skills: SkillMetadata[] = [
      {
        name: 'workspace-skill',
        description: 'Workspace scoped skill',
        path: '/tmp/workspace/skills/workspace-skill.md',
        source: 'workspace',
        origin: 'workspace',
      },
      {
        name: 'assistant-skill',
        description: 'Assistant scoped skill',
        path: '/tmp/assistant/skills/assistant-skill.md',
        source: 'assistant',
        origin: 'assistant',
      },
      {
        name: 'user-skill',
        description: 'User scoped skill',
        path: '/tmp/user/skills/user-skill.md',
        source: 'global',
        origin: 'user',
      },
      {
        name: 'system-skill',
        description: 'System scoped skill',
        path: '/tmp/system/skills/system-skill.md',
        source: 'global',
        origin: 'system',
      },
    ];

    render(
      <InputTokenDropdown
        mode={{ kind: 'skills', items: skills }}
        onSelectType={vi.fn()}
        onSelectArg={onSelectArg}
        onDismiss={vi.fn()}
      />,
    );

    expect(screen.getByText('@skill:workspace-skill')).toBeInTheDocument();
    expect(screen.getByText('Workspace')).toBeInTheDocument();
    expect(screen.getByText('Assistant')).toBeInTheDocument();
    expect(screen.getByText('User')).toBeInTheDocument();
    expect(screen.getByText('System')).toBeInTheDocument();

    fireEvent.mouseDown(screen.getByText('@skill:workspace-skill'));

    expect(onSelectArg).toHaveBeenCalledWith('workspace-skill');
  });
});
