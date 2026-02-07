import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import FileAttachment from '../FileAttachment';

// Mock Button to avoid shadcn complexity if needed, but integration test is better
// However, Button uses Radix Slot, which might be complex.
// For now, let's try rendering the real component.

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
    expect(screen.getByRole('button', { name: 'Remove test.txt' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Remove image.png' })).toBeInTheDocument();
  });
});
