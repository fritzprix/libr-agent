import React, { memo, useState } from 'react';
import { useTranslation } from 'react-i18next';
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

export const ErrorBubble: React.FC<ErrorBubbleProps> = memo(
  ({ error, onRetry }) => {
    const { t } = useTranslation('common');
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
          return <Wifi size={16} className="text-warning" />;
        case 'MALFORMED_FUNCTION_CALL':
          return <Wrench size={16} className="text-primary" />;
        case 'JSON_PARSING_ERROR':
          return <FileX size={16} className="text-destructive" />;
        case 'AUTHENTICATION_ERROR':
          return <Key size={16} className="text-destructive" />;
        case 'RATE_LIMIT_ERROR':
          return <Clock size={16} className="text-warning" />;
        default:
          return <AlertTriangle size={16} className="text-destructive" />;
      }
    };

    const getErrorColor = (errorType: string) => {
      switch (errorType) {
        case 'NETWORK_ERROR':
          return 'border-warning/20 bg-warning/5';
        case 'MALFORMED_FUNCTION_CALL':
          return 'border-primary/20 bg-primary/5';
        case 'JSON_PARSING_ERROR':
          return 'border-destructive/20 bg-destructive/5';
        case 'AUTHENTICATION_ERROR':
          return 'border-destructive/20 bg-destructive/5';
        case 'RATE_LIMIT_ERROR':
          return 'border-warning/20 bg-warning/5';
        default:
          return 'border-destructive/20 bg-destructive/5';
      }
    };

    const getErrorBadgeColor = (errorType: string) => {
      switch (errorType) {
        case 'NETWORK_ERROR':
          return 'bg-warning text-warning-foreground';
        case 'MALFORMED_FUNCTION_CALL':
          return 'bg-primary text-primary-foreground';
        case 'JSON_PARSING_ERROR':
          return 'bg-destructive text-destructive-foreground';
        case 'AUTHENTICATION_ERROR':
          return 'bg-destructive text-destructive-foreground';
        case 'RATE_LIMIT_ERROR':
          return 'bg-warning text-warning-foreground';
        default:
          return 'bg-destructive text-destructive-foreground';
      }
    };

    logger.info('error : ', { error });
    return (
      <BaseBubble
        title={t('errorBubble.title', 'Error')}
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
          <p className="text-muted-foreground break-words whitespace-pre-wrap">
            {error?.displayMessage ||
              t('errorBubble.unknownError', 'An unknown error occurred.')}
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
                  {t('errorBubble.retrying', 'Retrying...')}
                </>
              ) : (
                <>
                  <RefreshCw className="w-4 h-4" />
                  {t('errorBubble.tryAgain', 'Try Again')}
                </>
              )}
            </Button>
          )}
        </div>
      </BaseBubble>
    );
  },
);

ErrorBubble.displayName = 'ErrorBubble';
