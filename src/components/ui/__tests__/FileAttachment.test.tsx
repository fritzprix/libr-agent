import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import FileAttachment from '../FileAttachment';

// Mock Button to avoid shadcn complexity if needed, but integration test is better
// However, Button uses Radix Slot, which might be complex.
// For now, let's try rendering the real component.

// Mock react-i18next to simulate interpolation
// Mock react-i18next to simulate interpolation
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown> | string) => {
      // Custom mock to simulate { name: 'filename.txt' } interpolation
      if (typeof options === 'object' && options && 'name' in options && typeof options.defaultValue === 'string') {
        return options.defaultValue.replace('{{name}}', options.name as string);
      }
      // If it's a simple string or single string arg fallback
      if (typeof options === 'string') return options;
      // Default return the key if nothing matches
      return key;
    },
  }),
}));

describe('FileAttachment', () => {
  const mockFiles = [
    { name: 'test.txt', content: 'hello' },
    { name: 'image.png', content: 'world' },
  ];
  const mockOnRemove = vi.fn();
  const mockOnAdd = vi.fn();

  it('renders attached files', () => {
    render(
      <FileAttachment
        files={mockFiles}
        onRemove={mockOnRemove}
        onAdd={mockOnAdd}
      />
    );

    expect(screen.getByText('test.txt')).toBeInTheDocument();
    expect(screen.getByText('image.png')).toBeInTheDocument();
  });

  it('buttons should have accessible labels', () => {
    render(
      <FileAttachment
        files={mockFiles}
        onRemove={mockOnRemove}
        onAdd={mockOnAdd}
      />
    );

    // Remove buttons should have specific, file-related accessible names for screen readers.
    // We mocked react-i18next to correctly substitute the {{name}} parameter.
    const listItems = screen.getAllByRole('listitem');
    expect(listItems.length).toBe(2);

    expect(screen.getByRole('button', { name: 'Remove test.txt' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Remove image.png' })).toBeInTheDocument();
  });
});
