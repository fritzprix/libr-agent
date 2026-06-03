import { useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/utils';
import { useListNavigation } from '@/hooks/use-list-navigation';
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
  const { t } = useTranslation();
  const count = mode.items.length;
  const itemRefs = useRef<Array<HTMLLIElement | null>>([]);
  const { kind, items } = mode;

  const handleEnter = useCallback(
    (index: number) => {
      if (kind === 'types') {
        const item = items[index];
        if (item) onSelectType(item.name);
      } else if (kind === 'files') {
        const item = items[index];
        if (item) onSelectArg(item);
      } else if (kind === 'tools') {
        const item = items[index];
        if (item) onSelectArg(item.name);
      } else if (kind === 'playbooks') {
        const item = items[index];
        if (item) onSelectArg(item.goal);
      } else {
        const item = items[index];
        if (item) onSelectArg(item.name);
      }
    },
    [kind, items, onSelectType, onSelectArg],
  );

  const { activeIndex, setActiveIndex } = useListNavigation({
    itemCount: count,
    onEnter: handleEnter,
    onEscape: onDismiss,
    resetDependencies: [kind, items],
  });

  const getSkillSourceLabel = (skill: SkillMetadata) => {
    switch (skill.origin) {
      case 'workspace':
        return t('mentions.skillSourceWorkspace', 'Workspace');
      case 'assistant':
        return t('mentions.skillSourceAssistant', 'Assistant');
      case 'user':
        return t('mentions.skillSourceUser', 'User');
      case 'system':
        return t('mentions.skillSourceSystem', 'System');
      default:
        switch (skill.source) {
          case 'workspace':
            return t('mentions.skillSourceWorkspace', 'Workspace');
          case 'assistant':
            return t('mentions.skillSourceAssistant', 'Assistant');
          case 'global':
            return t('mentions.skillSourceGlobal', 'Global');
          default:
            return null;
        }
    }
  };

  useEffect(() => {
    const activeItem = itemRefs.current[activeIndex];
    if (typeof activeItem?.scrollIntoView === 'function') {
      activeItem.scrollIntoView({ block: 'nearest' });
    }
  }, [activeIndex]);

  if (!count) return null;

  return (
    <ul
      role="listbox"
      aria-label={t(
        'agent.input.tokenSuggestionsAria',
        'Input token suggestions',
      )}
      className="absolute bottom-full left-0 mb-1 z-50 w-80 max-h-60 overflow-y-auto rounded-md border border-border bg-popover text-popover-foreground shadow-md text-sm"
    >
      {mode.kind === 'types'
        ? mode.items.map((type, i) => (
            <li
              key={type.name}
              ref={(element) => {
                itemRefs.current[i] = element;
              }}
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
                ref={(element) => {
                  itemRefs.current[i] = element;
                }}
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
                  ref={(element) => {
                    itemRefs.current[i] = element;
                  }}
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
                    ref={(element) => {
                      itemRefs.current[i] = element;
                    }}
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
                        {t('agent.input.stepsCount', {
                          count: playbook.workflow.length,
                        })}
                      </span>
                    )}
                  </li>
                ))
              : mode.items.map((skill, i) => {
                  const sourceLabel = getSkillSourceLabel(skill);

                  return (
                    <li
                      key={skill.name}
                      ref={(element) => {
                        itemRefs.current[i] = element;
                      }}
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
                      <span className="flex items-center justify-between gap-2">
                        <span className="min-w-0 truncate font-medium">
                          @skill:{skill.name}
                        </span>
                        {sourceLabel && (
                          <span className="shrink-0 rounded-full border px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
                            {sourceLabel}
                          </span>
                        )}
                      </span>
                      {skill.description && (
                        <span className="text-xs text-muted-foreground line-clamp-1">
                          {skill.description}
                        </span>
                      )}
                    </li>
                  );
                })}
    </ul>
  );
}
