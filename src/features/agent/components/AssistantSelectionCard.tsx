import { cn } from '@/lib/utils';
import { Bot } from 'lucide-react';
import type { Assistant } from '@/models/chat';

interface AssistantSelectionCardProps {
  assistant: Assistant;
  isStarting: boolean;
  disabled: boolean;
  onSelect: (assistant: Assistant) => void;
}

export function AssistantSelectionCard({
  assistant,
  isStarting,
  disabled,
  onSelect,
}: AssistantSelectionCardProps) {
  return (
    <button
      onClick={() => onSelect(assistant)}
      disabled={disabled}
      aria-label={`Start session with ${assistant.name}`}
      aria-busy={isStarting}
      aria-disabled={disabled}
      className={cn(
        'group w-full p-5 text-left border rounded-2xl transition-all duration-200',
        'flex flex-col gap-3',
        'hover:shadow-lg hover:bg-accent/40 hover:-translate-y-0.5',
        'focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
        disabled && !isStarting && 'opacity-50 cursor-not-allowed',
        isStarting && 'border-primary bg-primary/10 animate-pulse',
        !disabled && !isStarting && 'hover:border-primary/40',
      )}
    >
      {/* Icon + Name Row */}
      <div className="flex items-center gap-3">
        <div
          className={cn(
            'w-10 h-10 rounded-xl flex items-center justify-center flex-shrink-0 transition-colors',
            isStarting
              ? 'bg-primary/20 text-primary'
              : 'bg-muted group-hover:bg-primary/10 group-hover:text-primary',
          )}
        >
          <Bot size={20} />
        </div>
        <div className="flex-1 min-w-0">
          <div className="font-semibold text-base leading-tight truncate flex items-center gap-2">
            {assistant.name}
            {isStarting && (
              <span className="text-xs text-primary font-normal animate-pulse">
                Starting...
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Description */}
      {assistant.description && (
        <p className="text-sm text-muted-foreground leading-relaxed line-clamp-2">
          {assistant.description}
        </p>
      )}
    </button>
  );
}
