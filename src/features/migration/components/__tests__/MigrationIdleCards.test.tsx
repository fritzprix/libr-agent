import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom';
import { MigrationIdleCards } from '../MigrationIdleCards';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, defaultValue?: string) =>
      typeof defaultValue === 'string' ? defaultValue : _key,
  }),
}));

describe('MigrationIdleCards', () => {
  it('toggles sensitive data once when clicking the checkbox itself', () => {
    const setIncludeSensitiveData = vi.fn();

    render(
      <MigrationIdleCards
        includeSensitiveData={false}
        setIncludeSensitiveData={setIncludeSensitiveData}
        selectedExportDir={null}
        selectExportFile={vi.fn()}
        selectImportFile={vi.fn()}
        onExport={vi.fn()}
      />,
    );

    const checkbox = screen.getByRole('checkbox');
    fireEvent.click(checkbox);

    expect(setIncludeSensitiveData).toHaveBeenCalledTimes(1);
    expect(setIncludeSensitiveData).toHaveBeenCalledWith(true);
  });

  it('toggles sensitive data when clicking the surrounding row', () => {
    const setIncludeSensitiveData = vi.fn();

    render(
      <MigrationIdleCards
        includeSensitiveData={false}
        setIncludeSensitiveData={setIncludeSensitiveData}
        selectedExportDir={null}
        selectExportFile={vi.fn()}
        selectImportFile={vi.fn()}
        onExport={vi.fn()}
      />,
    );

    fireEvent.click(
      screen.getByText('Include sensitive data (full Settings table)'),
    );

    expect(setIncludeSensitiveData).toHaveBeenCalledTimes(1);
    expect(setIncludeSensitiveData).toHaveBeenCalledWith(true);
  });
});
