import React, { memo, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
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
  Settings,
} from 'lucide-react';
import { getLogger } from '@/lib/logger';
import { cn } from '@/lib/utils';

const logger = getLogger('ErrorBubble');

interface ErrorBubbleProps {
  // New: allow passing only the error object (transient UI error state).
  error?: Message['error'] | null;
  onRetry?: () => Promise<void>;
}

export const ErrorBubble: React.FC<ErrorBubbleProps> = memo(
  ({ error, onRetry }) => {
    const { t } = useTranslation('common');
    const navigate = useNavigate();
    const [retrying, setRetrying] = useState(false);
    const lastLoggedErrorKeyRef = useRef<string | null>(null);

    useEffect(() => {
      if (!error) {
        lastLoggedErrorKeyRef.current = null;
        return;
      }

      const errorKey =
        error.details?.timestamp ??
        `${error.type}:${error.displayMessage}:${error.recoverable}`;

      if (lastLoggedErrorKeyRef.current === errorKey) {
        return;
      }

      lastLoggedErrorKeyRef.current = errorKey;
      logger.info('Rendering error bubble', { error });
    }, [error]);

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
          return 'border-warning/30 border-l-4 border-l-warning bg-warning/5';
        case 'MALFORMED_FUNCTION_CALL':
          return 'border-primary/30 border-l-4 border-l-primary bg-primary/5';
        case 'JSON_PARSING_ERROR':
          return 'border-destructive/30 border-l-4 border-l-destructive bg-destructive/5';
        case 'AUTHENTICATION_ERROR':
          return 'border-destructive/30 border-l-4 border-l-destructive bg-destructive/5';
        case 'RATE_LIMIT_ERROR':
          return 'border-warning/30 border-l-4 border-l-warning bg-warning/5';
        default:
          return 'border-destructive/30 border-l-4 border-l-destructive bg-destructive/5';
      }
    };

    const getErrorBadgeColor = (errorType: string) => {
      switch (errorType) {
        case 'NETWORK_ERROR':
          return 'border border-warning/30 bg-warning/15 text-warning';
        case 'MALFORMED_FUNCTION_CALL':
          return 'border border-primary/30 bg-primary/15 text-primary';
        case 'JSON_PARSING_ERROR':
          return 'border border-destructive/30 bg-destructive/15 text-destructive';
        case 'AUTHENTICATION_ERROR':
          return 'border border-destructive/30 bg-destructive/15 text-destructive';
        case 'RATE_LIMIT_ERROR':
          return 'border border-warning/30 bg-warning/15 text-warning';
        default:
          return 'border border-destructive/30 bg-destructive/15 text-destructive';
      }
    };

    const errorType = error?.type || 'UNKNOWN_ERROR';

    return (
      <BaseBubble
        title={t('errorBubble.title', 'Error')}
        defaultExpanded={true}
        icon={getErrorIcon(errorType)}
        badge={
          <span
            className={cn(
              'rounded-md px-2 py-0.5 text-xs font-medium',
              getErrorBadgeColor(errorType),
            )}
          >
            {error?.type}
          </span>
        }
        className={getErrorColor(errorType)}
      >
        <div className="space-y-3">
          <p className="text-muted-foreground break-words whitespace-pre-wrap">
            {error?.displayMessage ||
              t('errorBubble.unknownError', 'An unknown error occurred.')}
          </p>

          {errorType === 'AUTHENTICATION_ERROR' && (
            <Button
              onClick={() => navigate('/settings')}
              variant="outline"
              size="sm"
              className="flex items-center gap-2"
            >
              <Settings className="w-4 h-4" />
              {t('errorBubble.goToSettings', 'Configure API Key in Settings')}
            </Button>
          )}

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
