import React from 'react';
import { Paperclip, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';

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
        <Button
          variant="ghost"
          size="icon"
          type="button"
          onClick={handleFileSelect}
          className="h-8 w-8 text-gray-500 hover:text-green-400"
          title="Attach files"
        >
          <Paperclip className="h-4 w-4" />
        </Button>

        {/* File Count Indicator */}
        {files.length > 0 && (
          <span className="text-xs text-gray-400">{files.length}</span>
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
      <Button
        variant="ghost"
        size="sm"
        type="button"
        onClick={handleFileSelect}
        className="text-gray-500 hover:text-green-400 border border-gray-600"
        title="Attach files"
      >
        <Paperclip className="h-4 w-4" />
      </Button>

      {/* Attached Files Display */}
      {files.length > 0 && (
        <div className="mt-2">
          <div className="text-xs text-gray-500 mb-2">📎 Attached Files:</div>
          <div className="space-y-1">
            {files.map((file, index) => (
              <div
                key={index}
                className="flex items-center justify-between bg-gray-900 px-2 py-1 rounded border border-gray-700"
              >
                <span className="text-xs text-green-400 truncate flex-1">
                  {file.name}
                </span>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  onClick={() => onRemove(index)}
                  className="h-6 w-6 ml-2 text-red-400 hover:text-red-300"
                  title="Remove file"
                >
                  <Trash2 className="h-3 w-3" />
                </Button>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
