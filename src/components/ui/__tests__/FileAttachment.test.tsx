import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import FileAttachment from '../FileAttachment';
import React from 'react';

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

    // Current behavior: they have title="Remove file", so name is "Remove file"
    // We want them to have specific names like "Remove test.txt"

    // This expects the FUTURE behavior, so it should FAIL now
    expect(screen.getByRole('button', { name: 'Remove test.txt' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Remove image.png' })).toBeInTheDocument();
  });
});
