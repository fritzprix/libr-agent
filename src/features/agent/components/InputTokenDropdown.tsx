import { useEffect, useRef, useState } from 'react';
import { cn } from '@/lib/utils';
import type { TokenType } from '../hooks/useInputToken';
import type { SkillMetadata } from '@/types/skills';
import type { MCPTool } from '@/lib/mcp';
import type { Playbook } from '@/types/playbook';

type DropdownMode =
  | { kind: 'types'; items: TokenType[] }
  | { kind: 'skills'; items: SkillMetadata[] }
  | { kind: 'tools'; items: MCPTool[] }
  | { kind: 'files'; items: string[] }
  | {
      kind: 'playbooks';
      items: (Playbook & { id: string; createdAt: Date; updatedAt: Date })[];
    };

interface InputTokenDropdownProps {
  mode: DropdownMode;
  onSelectType: (typeName: string) => void;
  onSelectArg: (arg: string) => void;
  onDismiss: () => void;
}

export function InputTokenDropdown({
  mode,
  onSelectType,
  onSelectArg,
  onDismiss,
}: InputTokenDropdownProps) {
  const count = mode.items.length;
  const [activeIndex, setActiveIndex] = useState(0);
  const listRef = useRef<HTMLUListElement>(null);

  useEffect(() => {
    setActiveIndex(0);
  }, [mode]);

  useEffect(() => {
    if (!count) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setActiveIndex((i) => Math.min(i + 1, count - 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setActiveIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault();
        if (mode.kind === 'types') {
          const item = mode.items[activeIndex];
          if (item) onSelectType(item.name);
        } else if (mode.kind === 'files') {
          const item = mode.items[activeIndex];
          if (item) onSelectArg(item);
        } else if (mode.kind === 'tools') {
          const item = mode.items[activeIndex];
          if (item) onSelectArg(item.name);
        } else if (mode.kind === 'playbooks') {
          const item = mode.items[activeIndex];
          if (item) onSelectArg(item.goal);
        } else {
          const item = mode.items[activeIndex];
          if (item) onSelectArg(item.name);
        }
      } else if (e.key === 'Escape') {
        e.preventDefault();
        onDismiss();
      }
    };
    window.addEventListener('keydown', handler, { capture: true });
    return () =>
      window.removeEventListener('keydown', handler, { capture: true });
  }, [count, mode, activeIndex, onSelectType, onSelectArg, onDismiss]);

  if (!count) return null;

  return (
    <ul
      ref={listRef}
      role="listbox"
      aria-label="Input token suggestions"
      className="absolute bottom-full left-0 mb-1 z-50 w-80 max-h-60 overflow-y-auto rounded-md border border-border bg-popover text-popover-foreground shadow-md text-sm"
    >
      {mode.kind === 'types'
        ? mode.items.map((type, i) => (
            <li
              key={type.name}
              role="option"
              aria-selected={i === activeIndex}
              className={cn(
                'flex flex-col gap-0.5 px-3 py-2 cursor-pointer select-none',
                i === activeIndex
                  ? 'bg-accent text-accent-foreground'
                  : 'hover:bg-accent/50',
              )}
              onMouseEnter={() => setActiveIndex(i)}
              onMouseDown={(e) => {
                e.preventDefault();
                onSelectType(type.name);
              }}
            >
              <span className="font-medium font-mono">{type.label}</span>
              <span className="text-xs text-muted-foreground">
                {type.description}
              </span>
            </li>
          ))
        : mode.kind === 'tools'
          ? mode.items.map((tool, i) => (
              <li
                key={tool.name}
                role="option"
                aria-selected={i === activeIndex}
                className={cn(
                  'flex flex-col gap-0.5 px-3 py-2 cursor-pointer select-none',
                  i === activeIndex
                    ? 'bg-accent text-accent-foreground'
                    : 'hover:bg-accent/50',
                )}
                onMouseEnter={() => setActiveIndex(i)}
                onMouseDown={(e) => {
                  e.preventDefault();
                  onSelectArg(tool.name);
                }}
              >
                <span className="font-medium font-mono">@tool:{tool.name}</span>
                {tool.description && (
                  <span className="text-xs text-muted-foreground line-clamp-1">
                    {tool.description}
                  </span>
                )}
              </li>
            ))
          : mode.kind === 'files'
            ? mode.items.map((filePath, i) => (
                <li
                  key={filePath}
                  role="option"
                  aria-selected={i === activeIndex}
                  className={cn(
                    'flex flex-col gap-0.5 px-3 py-2 cursor-pointer select-none',
                    i === activeIndex
                      ? 'bg-accent text-accent-foreground'
                      : 'hover:bg-accent/50',
                  )}
                  onMouseEnter={() => setActiveIndex(i)}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    onSelectArg(filePath);
                  }}
                >
                  <span className="font-medium font-mono text-xs truncate">
                    @file:{filePath}
                  </span>
                </li>
              ))
            : mode.kind === 'playbooks'
              ? mode.items.map((playbook, i) => (
                  <li
                    key={playbook.id}
                    role="option"
                    aria-selected={i === activeIndex}
                    className={cn(
                      'flex flex-col gap-0.5 px-3 py-2 cursor-pointer select-none',
                      i === activeIndex
                        ? 'bg-accent text-accent-foreground'
                        : 'hover:bg-accent/50',
                    )}
                    onMouseEnter={() => setActiveIndex(i)}
                    onMouseDown={(e) => {
                      e.preventDefault();
                      onSelectArg(playbook.goal);
                    }}
                  >
                    <span className="font-medium font-mono text-xs truncate">
                      @playbook:{playbook.goal}
                    </span>
                    {playbook.workflow.length > 0 && (
                      <span className="text-xs text-muted-foreground">
                        {playbook.workflow.length} step
                        {playbook.workflow.length !== 1 ? 's' : ''}
                      </span>
                    )}
                  </li>
                ))
              : mode.items.map((skill, i) => (
                  <li
                    key={skill.name}
                    role="option"
                    aria-selected={i === activeIndex}
                    className={cn(
                      'flex flex-col gap-0.5 px-3 py-2 cursor-pointer select-none',
                      i === activeIndex
                        ? 'bg-accent text-accent-foreground'
                        : 'hover:bg-accent/50',
                    )}
                    onMouseEnter={() => setActiveIndex(i)}
                    onMouseDown={(e) => {
                      e.preventDefault();
                      onSelectArg(skill.name);
                    }}
                  >
                    <span className="font-medium">@skill:{skill.name}</span>
                    {skill.description && (
                      <span className="text-xs text-muted-foreground line-clamp-1">
                        {skill.description}
                      </span>
                    )}
                  </li>
                ))}
    </ul>
  );
}
