import { useCallback, useRef } from 'react';
import type { ChangeEvent, KeyboardEvent } from 'react';
import { useSkills } from '@/context/SkillsContext';
import { useInputToken } from '@/features/agent/hooks/useInputToken';
import { InputTokenDropdown } from '@/features/agent/components/InputTokenDropdown';
import { usePlaybookSearch } from '@/features/agent/hooks/usePlaybookSearch';
import { Textarea } from '@/components/ui/textarea';
import { cn } from '@/lib/utils';

interface MentionTextareaProps {
  value: string;
  onChange: (value: string) => void;
  /** Assistant ID used to scope @playbook: completions */
  assistantId?: string;
  placeholder?: string;
  className?: string;
  rows?: number;
}

/**
 * Textarea with @mention autocomplete — same pattern as AgentChatInput.
 * handleKeyDown just returns early for navigation keys; InputTokenDropdown's
 * window capture listener handles the actual selection.
 */
export function MentionTextarea({
  value,
  onChange,
  assistantId,
  placeholder = 'Type a message… use @playbook: or @skill: for autocomplete',
  className,
  rows = 3,
}: MentionTextareaProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { skills } = useSkills();

  const {
    stage,
    typeResults,
    skillResults,
    onInputChange: onTokenInputChange,
    onTypeSelect,
    onArgSelect,
    onDismiss,
  } = useInputToken(skills, []);

  const playbookQuery =
    stage.kind === 'typing-arg' && stage.typeName === 'playbook'
      ? stage.query
      : null;
  const playbookResults = usePlaybookSearch(assistantId, playbookQuery);

  const isDropdownOpen =
    stage.kind !== 'idle' &&
    (typeResults.length > 0 ||
      skillResults.length > 0 ||
      playbookResults.length > 0);

  const dropdownMode =
    stage.kind === 'typing-type'
      ? { kind: 'types' as const, items: typeResults }
      : stage.kind === 'typing-arg' && stage.typeName === 'playbook'
        ? { kind: 'playbooks' as const, items: playbookResults }
        : stage.kind === 'typing-arg'
          ? { kind: 'skills' as const, items: skillResults }
          : null;

  const handleChange = useCallback(
    (e: ChangeEvent<HTMLTextAreaElement>) => {
      onChange(e.target.value);
      onTokenInputChange(
        e.target.value,
        e.target.selectionStart ?? e.target.value.length,
      );
    },
    [onChange, onTokenInputChange],
  );

  // Same pattern as AgentChatInput: just return, let InputTokenDropdown's
  // window capture listener handle Tab/Enter/Arrow/Escape.
  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (
        isDropdownOpen &&
        ['ArrowUp', 'ArrowDown', 'Enter', 'Tab', 'Escape'].includes(e.key)
      ) {
        return;
      }
    },
    [isDropdownOpen],
  );

  return (
    <div className="relative">
      {dropdownMode && (
        <InputTokenDropdown
          mode={dropdownMode}
          onSelectType={(typeName) => {
            const cursorPos =
              textareaRef.current?.selectionStart ?? value.length;
            const newValue = onTypeSelect(typeName, value, cursorPos);
            onChange(newValue);
            requestAnimationFrame(() => {
              if (textareaRef.current) {
                const pos = newValue.length - (value.length - cursorPos);
                textareaRef.current.setSelectionRange(pos, pos);
                textareaRef.current.focus();
              }
            });
          }}
          onSelectArg={(arg) => {
            const cursorPos =
              textareaRef.current?.selectionStart ?? value.length;
            const newValue = onArgSelect(arg, value, cursorPos);
            onChange(newValue);
            requestAnimationFrame(() => {
              textareaRef.current?.focus();
            });
          }}
          onDismiss={onDismiss}
        />
      )}
      <Textarea
        ref={textareaRef}
        value={value}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        rows={rows}
        className={cn('resize-none', className)}
      />
    </div>
  );
}
