import React, { useState } from 'react';
import { Message } from '@/models/chat';
import { BaseBubble } from '@/components/ui/BaseBubble';
import { Button } from '@/components/ui/button';
import {
  AlertTriangle,
  Wifi,
  Wrench,
  FileX,
  RefreshCw,
  Loader2,
  Key,
  Clock,
} from 'lucide-react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('ErrorBubble');

interface ErrorBubbleProps {
  // New: allow passing only the error object (transient UI error state).
  error?: Message['error'] | null;
  onRetry?: () => Promise<void>;
}

export const ErrorBubble: React.FC<ErrorBubbleProps> = ({ error, onRetry }) => {
  const [retrying, setRetrying] = useState(false);

  const handleRetry = async () => {
    if (!onRetry || !error?.recoverable) return;

    setRetrying(true);
    try {
      await onRetry();
    } finally {
      setRetrying(false);
    }
  };

  const getErrorIcon = (errorType: string) => {
    switch (errorType) {
      case 'NETWORK_ERROR':
        return <Wifi size={16} className="text-amber-600" />;
      case 'MALFORMED_FUNCTION_CALL':
        return <Wrench size={16} className="text-blue-600" />;
      case 'JSON_PARSING_ERROR':
        return <FileX size={16} className="text-destructive" />;
      case 'AUTHENTICATION_ERROR':
        return <Key size={16} className="text-red-600" />;
      case 'RATE_LIMIT_ERROR':
        return <Clock size={16} className="text-orange-600" />;
      default:
        return <AlertTriangle size={16} className="text-destructive" />;
    }
  };

  const getErrorColor = (errorType: string) => {
    switch (errorType) {
      case 'NETWORK_ERROR':
        return 'border-amber-200 bg-amber-50';
      case 'MALFORMED_FUNCTION_CALL':
        return 'border-blue-200 bg-blue-50';
      case 'JSON_PARSING_ERROR':
        return 'border-destructive/20 bg-destructive/5';
      case 'AUTHENTICATION_ERROR':
        return 'border-red-200 bg-red-50';
      case 'RATE_LIMIT_ERROR':
        return 'border-orange-200 bg-orange-50';
      default:
        return 'border-destructive/20 bg-destructive/5';
    }
  };

  const getErrorBadgeColor = (errorType: string) => {
    switch (errorType) {
      case 'NETWORK_ERROR':
        return 'bg-amber-600 text-white';
      case 'MALFORMED_FUNCTION_CALL':
        return 'bg-blue-600 text-white';
      case 'JSON_PARSING_ERROR':
        return 'bg-destructive text-destructive-foreground';
      case 'AUTHENTICATION_ERROR':
        return 'bg-red-600 text-white';
      case 'RATE_LIMIT_ERROR':
        return 'bg-orange-600 text-white';
      default:
        return 'bg-destructive text-destructive-foreground';
    }
  };

  logger.info('error : ', { error });
  return (
    <BaseBubble
      title="Error"
      defaultExpanded={true}
      icon={getErrorIcon(error?.type || 'UNKNOWN_ERROR')}
      badge={
        <span
          className={`px-2 py-1 text-xs rounded-full ${getErrorBadgeColor(error?.type || 'UNKNOWN_ERROR')}`}
        >
          {error?.type}
        </span>
      }
      className={getErrorColor(error?.type || 'UNKNOWN_ERROR')}
    >
      <div className="space-y-3">
        <p className="text-muted-foreground">
          {error?.displayMessage || 'An unknown error occurred.'}
        </p>

        {error?.recoverable && (
          <Button
            onClick={handleRetry}
            disabled={retrying}
            variant="outline"
            size="sm"
            className="flex items-center gap-2"
          >
            {retrying ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                Retrying...
              </>
            ) : (
              <>
                <RefreshCw className="w-4 h-4" />
                Try Again
              </>
            )}
          </Button>
        )}
      </div>
    </BaseBubble>
  );
};
