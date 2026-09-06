import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom';
import type { MigrationPreview } from '@/lib/backend/migration';
import { MigrationPreviewCard } from '../MigrationPreviewCard';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string | Record<string, unknown>) => {
      if (typeof defaultValue === 'string') return defaultValue;
      if (
        defaultValue &&
        typeof defaultValue === 'object' &&
        'defaultValue' in defaultValue
      ) {
        const template = String(defaultValue.defaultValue);
        return template.replace(
          /\{\{(\w+)\}\}/g,
          (_, name: string) => String(defaultValue[name] ?? ''),
        );
      }
      return key;
    },
  }),
}));

const compatiblePreview: MigrationPreview = {
  format_version: 1,
  app_version: '0.9.7',
  exported_at: '2026-09-01T12:00:00.000Z',
  compatibility: 'Compatible',
  sections: [
    { name: 'settings', item_count: 3, size_bytes: 2048 },
    { name: 'assistants', item_count: 1, size_bytes: 1024 },
  ],
  total_size_bytes: 3072,
  file_path: '/tmp/backup.libragent-migration',
};

const incompatiblePreview: MigrationPreview = {
  ...compatiblePreview,
  compatibility: {
    Incompatible: { message: 'Archive format is too new for this app.' },
  },
};

const newerPreview: MigrationPreview = {
  ...compatiblePreview,
  compatibility: {
    NewerVersion: { message: 'Backup was created by a newer app version.' },
  },
};

describe('MigrationPreviewCard', () => {
  it('renders archive path, section rows, and compatible badge', () => {
    render(
      <MigrationPreviewCard
        preview={compatiblePreview}
        strategy="skip"
        setStrategy={vi.fn()}
        onImport={vi.fn()}
        onReset={vi.fn()}
      />,
    );

    expect(
      screen.getByText('/tmp/backup.libragent-migration'),
    ).toBeInTheDocument();
    expect(screen.getByText('✅ Compatible')).toBeInTheDocument();
    expect(screen.getByText('0.9.7')).toBeInTheDocument();
    expect(screen.getByText('settings')).toBeInTheDocument();
    expect(screen.getByText('assistants')).toBeInTheDocument();
    expect(screen.getByText('3 items')).toBeInTheDocument();
    expect(screen.getByText('1 items')).toBeInTheDocument();
  });

  it('shows compatibility warning for newer archives', () => {
    render(
      <MigrationPreviewCard
        preview={newerPreview}
        strategy="skip"
        setStrategy={vi.fn()}
        onImport={vi.fn()}
        onReset={vi.fn()}
      />,
    );

    expect(screen.getByText('⚠️ Newer version warning')).toBeInTheDocument();
    expect(
      screen.getByText(/Backup was created by a newer app version/),
    ).toBeInTheDocument();
  });

  it('disables import when archive is incompatible', () => {
    render(
      <MigrationPreviewCard
        preview={incompatiblePreview}
        strategy="skip"
        setStrategy={vi.fn()}
        onImport={vi.fn()}
        onReset={vi.fn()}
      />,
    );

    expect(screen.getByText('❌ Incompatible')).toBeInTheDocument();
    expect(
      screen.getByText(/Archive format is too new for this app/),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Run Import/i })).toBeDisabled();
  });

  it('notifies strategy and import/reset actions', () => {
    const setStrategy = vi.fn();
    const onImport = vi.fn();
    const onReset = vi.fn();

    render(
      <MigrationPreviewCard
        preview={compatiblePreview}
        strategy="skip"
        setStrategy={setStrategy}
        onImport={onImport}
        onReset={onReset}
      />,
    );

    fireEvent.click(screen.getByText('Overwrite'));
    expect(setStrategy).toHaveBeenCalledWith('overwrite');

    fireEvent.click(screen.getByText('Merge'));
    expect(setStrategy).toHaveBeenCalledWith('merge');

    fireEvent.click(screen.getByRole('button', { name: /Run Import/i }));
    expect(onImport).toHaveBeenCalledTimes(1);

    fireEvent.click(
      screen.getByRole('button', { name: /Cancel and go back/i }),
    );
    expect(onReset).toHaveBeenCalledTimes(1);
  });
});
