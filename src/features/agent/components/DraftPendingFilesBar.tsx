import { useTranslation } from 'react-i18next';
import { Paperclip, X } from 'lucide-react';

interface DraftPendingFilesBarProps {
  pendingFiles: File[];
  onRemoveFile: (index: number) => void;
}

export function DraftPendingFilesBar({
  pendingFiles,
  onRemoveFile,
}: DraftPendingFilesBarProps) {
  const { t } = useTranslation();

  return (
    <div className="px-4 py-3 bg-background/60 backdrop-blur-md rounded-t-xl border-x border-t border-border/50 animate-in slide-in-from-bottom-2 duration-300">
      <div className="text-[10px] mb-2 flex items-center gap-1.5 font-bold text-muted-foreground font-sans uppercase tracking-widest">
        <Paperclip className="w-3.5 h-3.5" />
        <span>{t('agent.draft.attachedFiles', 'Attached Files')}:</span>
      </div>
      <ul className="flex flex-wrap gap-2">
        {pendingFiles.map((file, index) => (
          <li
            key={`${file.name}-${index}`}
            className="flex items-center gap-2 px-2.5 py-1.5 rounded-lg border border-border bg-background/50 shadow-sm transition-all hover:border-primary/30"
          >
            <span className="text-xs font-medium font-sans truncate max-w-[200px]">
              {file.name}
            </span>
            <button
              type="button"
              onClick={() => onRemoveFile(index)}
              className="text-muted-foreground hover:text-destructive transition-colors focus:outline-none"
              aria-label={t('fileAttachment.removeFile', {
                name: file.name,
                defaultValue: `Remove ${file.name}`,
              })}
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
