import { cn } from '@/lib/utils';
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
      role="listitem"
      className={cn(
        'w-full p-4 text-left border rounded-xl transition-all',
        'hover:shadow-md hover:bg-accent/50 focus:outline-none focus:ring-2 focus:ring-primary',
        disabled && !isStarting && 'opacity-50 cursor-not-allowed',
        isStarting && 'border-primary bg-primary/10 animate-pulse',
        !disabled && !isStarting && 'hover:border-primary/50',
      )}
    >
      <div className="flex items-start justify-between">
        <div className="flex-1">
          <div className="font-semibold text-lg flex items-center gap-2">
            {assistant.name}
            {isStarting && (
              <span className="text-sm text-primary font-normal">
                Starting...
              </span>
            )}
          </div>
          {assistant.systemPrompt && (
            <p className="text-sm text-muted-foreground mt-2 line-clamp-3">
              {assistant.systemPrompt}
            </p>
          )}
        </div>
      </div>
    </button>
  );
}
