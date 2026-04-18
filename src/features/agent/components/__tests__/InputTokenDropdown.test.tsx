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

function mockScrollIntoView() {
  Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
    value: vi.fn(),
    configurable: true,
    writable: true,
  });
}

describe('InputTokenDropdown', () => {
  it('renders skill source badges and inserts the selected skill', () => {
    mockScrollIntoView();

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

  it('scrolls the active option into view during keyboard navigation', () => {
    mockScrollIntoView();

    const options = Array.from({ length: 4 }, (_, index) => ({
      name: `type-${index}`,
      label: `@type-${index}`,
      description: `Option ${index}`,
    }));

    render(
      <InputTokenDropdown
        mode={{ kind: 'types', items: options }}
        onSelectType={vi.fn()}
        onSelectArg={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );

    const renderedOptions = screen.getAllByRole('option');
    const targetOption = renderedOptions[2];
    const scrollIntoView = vi.fn();
    Object.defineProperty(targetOption, 'scrollIntoView', {
      value: scrollIntoView,
      configurable: true,
    });

    fireEvent.keyDown(window, { key: 'ArrowDown' });
    fireEvent.keyDown(window, { key: 'ArrowDown' });

    expect(targetOption).toHaveAttribute('aria-selected', 'true');
    expect(scrollIntoView).toHaveBeenCalledWith({ block: 'nearest' });
  });

  it('avoids redundant scroll work when keyboard navigation hits list boundaries', () => {
    mockScrollIntoView();

    const options = [
      { name: 'type-a', label: '@type-a', description: 'Option A' },
      { name: 'type-b', label: '@type-b', description: 'Option B' },
    ];

    render(
      <InputTokenDropdown
        mode={{ kind: 'types', items: options }}
        onSelectType={vi.fn()}
        onSelectArg={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );

    const renderedOptions = screen.getAllByRole('option');
    const topOptionScroll = vi.fn();
    const bottomOptionScroll = vi.fn();
    Object.defineProperty(renderedOptions[0], 'scrollIntoView', {
      value: topOptionScroll,
      configurable: true,
    });
    Object.defineProperty(renderedOptions[1], 'scrollIntoView', {
      value: bottomOptionScroll,
      configurable: true,
    });

    fireEvent.keyDown(window, { key: 'ArrowUp' });
    expect(topOptionScroll).not.toHaveBeenCalled();

    fireEvent.keyDown(window, { key: 'ArrowDown' });
    expect(bottomOptionScroll).toHaveBeenCalledTimes(1);
    bottomOptionScroll.mockClear();

    fireEvent.keyDown(window, { key: 'ArrowDown' });
    expect(bottomOptionScroll).not.toHaveBeenCalled();
  });
});
