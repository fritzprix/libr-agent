import { useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Input, Label } from '@/components/ui';
import { Switch } from '@/components/ui/switch';
import { Plus, Trash2 } from 'lucide-react';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { KeyValuePair } from '../hooks/useMCPServerForm';

export interface TransportAdvancedFieldsProps {
  customHeaders: KeyValuePair[];
  handleAddHeader: () => void;
  handleRemoveHeader: (id: string) => void;
  handleUpdateHeader: (
    id: string,
    field: 'key' | 'value',
    value: string,
  ) => void;
  enableSSE: boolean;
  setEnableSSE: (enable: boolean) => void;
  /** Extra class on the outer stack (e.g. accordion inner padding). */
  className?: string;
}

/**
 * Shared HTTP transport advanced controls (custom headers + SSE).
 * Used both inside HttpForm's accordion and flat under the registry Advanced panel.
 */
export function TransportAdvancedFields({
  customHeaders,
  handleAddHeader,
  handleRemoveHeader,
  handleUpdateHeader,
  enableSSE,
  setEnableSSE,
  className = 'space-y-4',
}: TransportAdvancedFieldsProps) {
  const { t } = useTranslation('common');
  const isAddingRef = useRef(false);

  return (
    <div className={className}>
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <Label>
            {t('mcpServer.dialog.customHeadersLabel', 'Custom Headers')}
          </Label>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => {
              isAddingRef.current = true;
              handleAddHeader();
            }}
            className="h-7 text-xs"
          >
            <Plus className="w-3 h-3 mr-1" />{' '}
            {t('mcpServer.dialog.addHeader', 'Add Header')}
          </Button>
        </div>

        {customHeaders.length === 0 ? (
          <p className="text-xs text-muted-foreground italic py-1">
            {t(
              'mcpServer.dialog.noCustomHeaders',
              'No custom headers configured.',
            )}
          </p>
        ) : (
          <div className="space-y-2">
            {customHeaders.map((header, index, arr) => {
              const removeLabel = header.key
                ? t('mcpServer.dialog.removeHeader', {
                    key: header.key,
                    defaultValue: 'Remove header {{key}}',
                  })
                : t(
                    'mcpServer.dialog.removeUnnamedHeader',
                    'Remove unnamed header',
                  );
              return (
                <div key={header.id} className="flex gap-2 items-start">
                  <div className="flex-1">
                    <Input
                      ref={(el) => {
                        if (
                          index === arr.length - 1 &&
                          isAddingRef.current &&
                          el
                        ) {
                          el.focus();
                          isAddingRef.current = false;
                        }
                      }}
                      id={`header-key-${header.id}`}
                      placeholder={t(
                        'mcpServer.dialog.headerKeyPlaceholder',
                        'Key (e.g. User-Agent)',
                      )}
                      value={header.key}
                      onChange={(e) =>
                        handleUpdateHeader(header.id, 'key', e.target.value)
                      }
                      className="h-8 text-sm"
                      aria-label="Custom header key"
                    />
                  </div>
                  <div className="flex-1">
                    <Input
                      placeholder={t(
                        'mcpServer.dialog.headerValuePlaceholder',
                        'Value',
                      )}
                      value={header.value}
                      onChange={(e) =>
                        handleUpdateHeader(header.id, 'value', e.target.value)
                      }
                      className="h-8 text-sm"
                      aria-label="Custom header value"
                    />
                  </div>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        onClick={() => handleRemoveHeader(header.id)}
                        aria-label={removeLabel}
                        className="h-8 w-8 text-muted-foreground hover:text-destructive"
                      >
                        <Trash2 className="w-4 h-4" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>{removeLabel}</TooltipContent>
                  </Tooltip>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div className="flex items-center justify-between">
        <div className="space-y-0.5">
          <Label htmlFor="enable-sse">
            {t('mcpServer.dialog.sseLabel', 'Enable Server-Sent Events (SSE)')}
          </Label>
          <p className="text-xs text-muted-foreground">
            {t(
              'mcpServer.dialog.sseDesc',
              'Keep enabled for streaming responses. Disable for stateless HTTP.',
            )}
          </p>
        </div>
        <Switch
          id="enable-sse"
          checked={enableSSE}
          onCheckedChange={setEnableSSE}
        />
      </div>
    </div>
  );
}
