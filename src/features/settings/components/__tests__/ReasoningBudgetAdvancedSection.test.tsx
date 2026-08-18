import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import { describe, expect, it, vi } from 'vitest';
import { ReasoningBudgetAdvancedSection } from '../ReasoningBudgetAdvancedSection';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue || key,
  }),
}));

describe('ReasoningBudgetAdvancedSection', () => {
  it('renders collapsed by default when no budget or message is set', () => {
    render(
      <ReasoningBudgetAdvancedSection
        idPrefix="openai"
        reasoningBudget={undefined}
        reasoningBudgetMessage={undefined}
        onChange={vi.fn()}
      />,
    );

    expect(screen.getByRole('button', { name: /Advanced/i })).toHaveAttribute(
      'aria-expanded',
      'false',
    );
    expect(
      screen.queryByLabelText(/Reasoning budget \(tokens\)/i),
    ).not.toBeInTheDocument();
  });

  it('renders expanded on mount when reasoning budget is present', () => {
    render(
      <ReasoningBudgetAdvancedSection
        idPrefix="openai"
        reasoningBudget={2048}
        reasoningBudgetMessage={undefined}
        onChange={vi.fn()}
      />,
    );

    expect(screen.getByRole('button', { name: /Advanced/i })).toHaveAttribute(
      'aria-expanded',
      'true',
    );
    expect(
      screen.getByLabelText(/Reasoning budget \(tokens\)/i),
    ).toBeInTheDocument();
    expect(
      screen.getByLabelText(/Reasoning budget \(tokens\)/i),
    ).toHaveValue(2048);
  });

  it('toggles open/close on button click', () => {
    render(
      <ReasoningBudgetAdvancedSection
        idPrefix="openai"
        reasoningBudget={undefined}
        reasoningBudgetMessage={undefined}
        onChange={vi.fn()}
      />,
    );

    const toggleButton = screen.getByRole('button', { name: /Advanced/i });
    fireEvent.click(toggleButton);

    expect(toggleButton).toHaveAttribute('aria-expanded', 'true');
    expect(
      screen.getByLabelText(/Reasoning budget \(tokens\)/i),
    ).toBeInTheDocument();

    fireEvent.click(toggleButton);
    expect(toggleButton).toHaveAttribute('aria-expanded', 'false');
  });

  it('calls onChange with parsed positive integer and undefined when cleared', () => {
    const onChange = vi.fn();
    render(
      <ReasoningBudgetAdvancedSection
        idPrefix="custom-1"
        reasoningBudget={4096}
        reasoningBudgetMessage="stop"
        onChange={onChange}
      />,
    );

    const budgetInput = screen.getByLabelText(/Reasoning budget \(tokens\)/i);
    fireEvent.change(budgetInput, { target: { value: '8192' } });
    expect(onChange).toHaveBeenCalledWith({ reasoningBudget: 8192 });

    fireEvent.change(budgetInput, { target: { value: '' } });
    expect(onChange).toHaveBeenCalledWith({ reasoningBudget: undefined });

    const messageInput = screen.getByLabelText(/Budget exceeded message/i);
    fireEvent.change(messageInput, { target: { value: 'custom finish' } });
    expect(onChange).toHaveBeenCalledWith({
      reasoningBudgetMessage: 'custom finish',
    });
  });
});
