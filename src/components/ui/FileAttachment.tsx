import React from 'react';
import { Paperclip, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from '@/components/ui/tooltip';

interface FileAttachmentProps {
  files: { name: string; content: string }[];
  onRemove: (index: number) => void;
  onAdd: (e: React.ChangeEvent<HTMLInputElement>) => void;
  maxFileSize?: number;
  allowedExtensions?: string[];
  compact?: boolean;
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
}: FileAttachmentProps) {
  const fileInputRef = React.useRef<HTMLInputElement>(null);

  const handleFileSelect = () => {
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
          accept={allowedExtensions.map((ext) => `.${ext}`).join(',')}
          onChange={onAdd}
          className="hidden"
        />

        {/* Attach Files Button */}
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              type="button"
              onClick={handleFileSelect}
              className="h-8 w-8 text-muted-foreground hover:text-success"
              aria-label="Attach files"
            >
              <Paperclip className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Attach files</TooltipContent>
        </Tooltip>

        {/* File Count Indicator */}
        {files.length > 0 && (
          <span className="text-xs text-muted-foreground">{files.length}</span>
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
        accept={allowedExtensions.map((ext) => `.${ext}`).join(',')}
        onChange={onAdd}
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
            className="text-muted-foreground hover:text-success border border-muted"
            aria-label="Attach files"
          >
            <Paperclip className="h-4 w-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Attach files</TooltipContent>
      </Tooltip>

      {/* Attached Files Display */}
      {files.length > 0 && (
        <div className="mt-2">
          <div className="text-xs text-muted-foreground mb-2 flex items-center gap-1">
            <Paperclip className="w-3 h-3" />
            <span>Attached Files:</span>
          </div>
          <ul className="space-y-1" aria-label="Attached files">
            {files.map((file, index) => (
              <li
                key={index}
                className="flex items-center justify-between bg-muted px-2 py-1 rounded border border-border"
              >
                <span className="text-xs text-success truncate flex-1">
                  {file.name}
                </span>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      onClick={() => onRemove(index)}
                      className="h-6 w-6 ml-2 text-destructive hover:text-destructive/80"
                      aria-label={`Remove ${file.name}`}
                    >
                      <Trash2 className="h-3 w-3" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>Remove {file.name}</TooltipContent>
                </Tooltip>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
