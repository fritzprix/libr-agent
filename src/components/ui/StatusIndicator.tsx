interface StatusIndicatorProps {
  status: 'connected' | 'disconnected' | 'unknown' | 'connecting';
  label?: string;
  showLabel?: boolean;
  size?: 'sm' | 'md' | 'lg';
}

export default function StatusIndicator({
  status,
  label,
  showLabel = false,
  size = 'md',
}: StatusIndicatorProps) {
  const statusColors = {
    connected: 'bg-green-400',
    disconnected: 'bg-red-400',
    unknown: 'bg-gray-400',
    connecting: 'bg-yellow-400 animate-pulse',
  };

  const statusTexts = {
    connected: 'Connected',
    disconnected: 'Disconnected',
    unknown: 'Unknown',
    connecting: 'Connecting...',
  };

  const sizeClasses = {
    sm: 'w-2 h-2',
    md: 'w-3 h-3',
    lg: 'w-4 h-4',
  };

  const displayLabel = label || statusTexts[status];

  return (
    <div className="flex items-center gap-1">
      <div
        className={`rounded-full ${statusColors[status]} ${sizeClasses[size]}`}
        title={displayLabel}
      />
      {showLabel && (
        <span className="text-xs text-gray-400">{displayLabel}</span>
      )}
    </div>
  );
}
