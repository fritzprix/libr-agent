import { cn } from '@/lib/utils';
import { Bot } from 'lucide-react';
import type { AssistantSummary } from '@/lib/backend/assistants';

interface AssistantSelectionCardProps {
  assistant: AssistantSummary;
  isStarting: boolean;
  disabled: boolean;
  onSelect: (assistant: AssistantSummary) => void;
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
        'group w-full p-6 text-left border rounded-[1.5rem] transition-all duration-300 relative overflow-hidden',
        'flex flex-col gap-4 bg-background/50 backdrop-blur-sm',
        'hover:shadow-2xl hover:bg-background hover:-translate-y-1',
        'focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
        disabled && !isStarting && 'opacity-50 cursor-not-allowed',
        isStarting && 'border-primary bg-primary/10 ring-2 ring-primary/20',
        !disabled && !isStarting && 'hover:border-primary/50 border-border/50',
      )}
    >
      {/* Subtle hover background effect */}
      <div className="absolute inset-0 bg-gradient-to-br from-primary/5 to-transparent opacity-0 group-hover:opacity-100 group-focus-visible:opacity-100 transition-opacity" />

      {/* Icon + Name Row */}
      <div className="flex items-center gap-4 relative z-10">
        <div
          className={cn(
            'w-12 h-12 rounded-2xl flex items-center justify-center flex-shrink-0 transition-all duration-300 shadow-sm',
            isStarting
              ? 'bg-primary text-primary-foreground scale-110'
              : 'bg-muted/50 text-muted-foreground group-hover:bg-primary/10 group-hover:text-primary group-hover:scale-110',
          )}
        >
          <Bot size={24} />
        </div>
        <div className="flex-1 min-w-0">
          <div className="font-bold text-lg leading-tight truncate flex items-center gap-2">
            {assistant.name}
            {isStarting && (
              <span className="text-[10px] text-primary uppercase font-bold tracking-widest animate-pulse ml-1">
                Starting
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Description */}
      {assistant.description && (
        <p className="text-sm text-muted-foreground/80 leading-relaxed line-clamp-2 font-sans relative z-10 px-0.5">
          {assistant.description}
        </p>
      )}
    </button>
  );
}
