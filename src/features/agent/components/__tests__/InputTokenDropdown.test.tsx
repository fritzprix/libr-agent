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
      },
      {
        name: 'assistant-skill',
        description: 'Assistant scoped skill',
        path: '/tmp/assistant/skills/assistant-skill.md',
        source: 'assistant',
      },
      {
        name: 'global-skill',
        description: 'Global scoped skill',
        path: '/tmp/global/skills/global-skill.md',
        source: 'global',
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
    expect(screen.getByText('workspace')).toBeInTheDocument();
    expect(screen.getByText('assistant')).toBeInTheDocument();
    expect(screen.getByText('global')).toBeInTheDocument();

    fireEvent.mouseDown(screen.getByText('@skill:workspace-skill'));

    expect(onSelectArg).toHaveBeenCalledWith('workspace-skill');
  });
});
