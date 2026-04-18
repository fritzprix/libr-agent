import '@testing-library/jest-dom/vitest';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import {
  formatSkillDisplayPath,
  SkillsListModal,
} from '../SkillsListModal';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (
      key: string,
      options?: string | { defaultValue?: string; count?: number },
    ) => {
      if (typeof options === 'string') {
        return options;
      }

      return options?.defaultValue ?? key;
    },
  }),
}));

describe('formatSkillDisplayPath', () => {
  it('removes the Windows extended-length drive prefix', () => {
    expect(
      formatSkillDisplayPath(
        '\\\\?\\C:\\Users\\SKTelecom\\my_works\\libr-agent\\SKILL.md',
      ),
    ).toBe('C:\\Users\\SKTelecom\\my_works\\libr-agent\\SKILL.md');
  });

  it('removes the Windows extended-length UNC prefix', () => {
    expect(
      formatSkillDisplayPath('\\\\?\\UNC\\server\\share\\skills\\SKILL.md'),
    ).toBe('\\\\server\\share\\skills\\SKILL.md');
  });
});

describe('SkillsListModal', () => {
  it('renders normalized skill paths', () => {
    render(
      <SkillsListModal
        isOpen={true}
        onClose={vi.fn()}
        systemSkills={[
          {
            name: 'deep-research-report',
            description: 'Bundled skill',
            path: '\\\\?\\C:\\Users\\SKTelecom\\my_works\\libr-agent\\src-tauri\\target\\debug\\bundled_skills\\deep-research-report\\SKILL.md',
            source: 'global',
            origin: 'system',
          },
        ]}
        userSkills={[]}
        onDeleteUserSkill={vi.fn()}
      />,
    );

    expect(
      screen.getByText(
        'C:\\Users\\SKTelecom\\my_works\\libr-agent\\src-tauri\\target\\debug\\bundled_skills\\deep-research-report\\SKILL.md',
      ),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(
        '\\\\?\\C:\\Users\\SKTelecom\\my_works\\libr-agent\\src-tauri\\target\\debug\\bundled_skills\\deep-research-report\\SKILL.md',
      ),
    ).not.toBeInTheDocument();
  });
});
