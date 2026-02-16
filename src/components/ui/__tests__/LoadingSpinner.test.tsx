import { render, screen } from '@testing-library/react';
import LoadingSpinner from '../LoadingSpinner';

describe('LoadingSpinner', () => {
  it('renders with role="status" and default label', () => {
    render(<LoadingSpinner />);
    const spinner = screen.getByRole('status');
    expect(spinner).toBeInTheDocument();
    expect(screen.getByText('Loading...')).toBeInTheDocument();
    expect(screen.getByText('Loading...')).toHaveClass('sr-only');
  });

  it('renders with custom label', () => {
    render(<LoadingSpinner label="Processing..." />);
    expect(screen.getByText('Processing...')).toBeInTheDocument();
  });

  it('renders with custom size and className', () => {
    const { container } = render(<LoadingSpinner size="sm" className="custom-class" />);
    // Check if the div has the custom class
    // Note: implementation details might vary, but we expect className to be applied
    expect(container.firstChild).toHaveClass('custom-class');
    expect(container.firstChild).toHaveClass('w-4 h-4');
  });
});
