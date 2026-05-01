import React from 'react';
import { useTranslation } from 'react-i18next';
import { Paperclip, Trash2, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';

interface FileAttachmentProps {
  files: { name: string; content: string; status?: string }[];
  onRemove?: (index: number) => void;
  onAdd: (e: React.ChangeEvent<HTMLInputElement>) => void;
  maxFileSize?: number;
  allowedExtensions?: string[];
  compact?: boolean;
  disabled?: boolean;
  showFileCount?: boolean;
  buttonClassName?: string;
}

export default function FileAttachment({
  files,
  onRemove,
  onAdd,
  allowedExtensions = [
    'txt',
    'md',
    'json',
    'js',
    'ts',
    'tsx',
    'jsx',
    'py',
    'java',
    'cpp',
    'c',
    'h',
    'css',
    'html',
    'xml',
    'yaml',
    'yml',
    'csv',
  ],
  compact = false,
  disabled = false,
  showFileCount = true,
  buttonClassName,
}: FileAttachmentProps) {
  const { t } = useTranslation('common');
  const fileInputRef = React.useRef<HTMLInputElement>(null);
  const accept =
    allowedExtensions.length > 0
      ? allowedExtensions.map((ext) => `.${ext}`).join(',')
      : undefined;

  const handleFileSelect = () => {
    if (disabled) return;
    fileInputRef.current?.click();
  };

  if (compact) {
    return (
      <div className="flex items-center gap-1">
        {/* File Input (Hidden) */}
        <input
          ref={fileInputRef}
          type="file"
          multiple
          accept={accept}
          onChange={onAdd}
          disabled={disabled}
          className="hidden"
        />

        {/* Attach Files Button */}
        <Tooltip>
          <TooltipTrigger asChild>
            <span
              tabIndex={disabled ? 0 : undefined}
              className={cn(
                'inline-block rounded-md focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
                disabled && 'cursor-not-allowed',
              )}
              aria-label={
                disabled
                  ? t('fileAttachment.attachFiles', 'Attach files')
                  : undefined
              }
              aria-disabled={disabled ? true : undefined}
              role={disabled ? 'button' : undefined}
            >
              <Button
                variant="ghost"
                size="icon"
                type="button"
                onClick={handleFileSelect}
                disabled={disabled}
                className={cn(
                  'h-8 w-8 text-muted-foreground hover:text-success',
                  disabled && 'pointer-events-none',
                  buttonClassName,
                )}
                aria-label={t('fileAttachment.attachFiles', 'Attach files')}
              >
                <Paperclip className="h-4 w-4" />
              </Button>
            </span>
          </TooltipTrigger>
          <TooltipContent>
            {t('fileAttachment.attachFiles', 'Attach files')}
          </TooltipContent>
        </Tooltip>

        {/* File Count Indicator */}
        {showFileCount && files.length > 0 && (
          <div className="flex items-center gap-1">
            {files.some((f) => f.status === 'processing') && (
              <Loader2 className="h-3 w-3 animate-spin text-muted-foreground" />
            )}
            <span className="text-xs text-muted-foreground">
              {files.length}
            </span>
          </div>
        )}
      </div>
    );
  }

  return (
    <div>
      {/* File Input (Hidden) */}
      <input
        ref={fileInputRef}
        type="file"
        multiple
        accept={accept}
        onChange={onAdd}
        disabled={disabled}
        className="hidden"
      />

      {/* Attach Files Button */}
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="sm"
            type="button"
            onClick={handleFileSelect}
            disabled={disabled}
            className={cn(
              'text-muted-foreground hover:text-success border border-muted',
              buttonClassName,
            )}
            aria-label={t('fileAttachment.attachFiles', 'Attach files')}
          >
            <Paperclip className="h-4 w-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          {t('fileAttachment.attachFiles', 'Attach files')}
        </TooltipContent>
      </Tooltip>

      {/* Attached Files Display */}
      {files.length > 0 && (
        <div className="mt-2">
          <div className="text-xs text-muted-foreground mb-2 flex items-center gap-1">
            <Paperclip className="w-3 h-3" />
            <span>
              {t('fileAttachment.attachedFilesLabel', 'Attached Files:')}
            </span>
          </div>
          <ul
            className="space-y-1"
            aria-label={t('fileAttachment.attachedFilesList', 'Attached files')}
          >
            {files.map((file, index) => {
              const removeLabel = t('fileAttachment.removeFile', {
                name: file.name,
                defaultValue: 'Remove {{name}}',
              });
              return (
                <li
                  key={file.name}
                  className="flex items-center justify-between bg-muted px-2 py-1 rounded border border-border"
                >
                  <span className="text-xs text-success truncate flex-1 flex items-center gap-2">
                    {file.status === 'processing' && (
                      <Loader2 className="h-3 w-3 animate-spin" />
                    )}
                    {file.name}
                  </span>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        onClick={() => onRemove?.(index)}
                        className="h-6 w-6 ml-2 text-destructive hover:text-destructive/80"
                        aria-label={removeLabel}
                      >
                        <Trash2 className="h-3 w-3" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>{removeLabel}</TooltipContent>
                  </Tooltip>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </div>
  );
}
