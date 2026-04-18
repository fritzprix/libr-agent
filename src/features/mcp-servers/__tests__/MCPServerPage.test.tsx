import '@testing-library/jest-dom/vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import MCPServerPage from '../MCPServerPage';

vi.mock('../MCPServerManagement', () => ({
  MCPServerManagement: () => <div>tools-panel</div>,
}));

vi.mock('@/features/skills/SkillsManagementPanel', () => ({
  SkillsManagementPanel: () => <div>skills-panel</div>,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (
      key: string,
      options?: string | { defaultValue?: string },
    ) => {
      if (typeof options === 'string') {
        return options;
      }

      return options?.defaultValue ?? key;
    },
  }),
}));

describe('MCPServerPage', () => {
  it('shows the tools view by default', () => {
    render(
      <MemoryRouter initialEntries={['/mcp-servers']}>
        <MCPServerPage />
      </MemoryRouter>,
    );

    expect(screen.getByRole('tab', { name: 'Tools' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Skills' })).toBeInTheDocument();
    expect(screen.getByText('tools-panel')).toBeInTheDocument();
    expect(screen.queryByText('skills-panel')).not.toBeInTheDocument();
  });

  it('opens the skills view from the query string', () => {
    render(
      <MemoryRouter initialEntries={['/mcp-servers?view=skills']}>
        <MCPServerPage />
      </MemoryRouter>,
    );

    expect(screen.getByText('skills-panel')).toBeInTheDocument();
    expect(screen.queryByText('tools-panel')).not.toBeInTheDocument();
  });
});
