import { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import {
  Copy,
  Check,
  ExternalLink,
  Eye,
  Code,
  AlertTriangle,
  Loader2,
} from 'lucide-react';
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from '@/components/ui/sheet';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { toast } from 'sonner';
import { cn } from '@/lib/utils';
import { useRustBackend } from '@/hooks/use-rust-backend';
import type { WorkspaceFileContent } from '@/lib/backend';
import type { FileNode } from './types';
import {
  getFileIconInfo,
  getFileExtension,
  getLanguageFromFileName,
} from './fileIconUtils';
import { CodeBlock } from '@/features/agent/components/AgentMessageRenderer/components/CodeBlock';

export interface WorkspaceFilePreviewSheetProps {
  file: FileNode | null;
  sessionId?: string;
  isOpen: boolean;
  onClose: () => void;
  onOpenInDefaultApp: (path: string) => Promise<void>;
}

function formatFileSize(bytes?: number | null): string {
  if (bytes === undefined || bytes === null) return '';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export const WorkspaceFilePreviewSheet = ({
  file,
  sessionId,
  isOpen,
  onClose,
  onOpenInDefaultApp,
}: WorkspaceFilePreviewSheetProps) => {
  const { t } = useTranslation();
  const { readWorkspaceFileContent } = useRustBackend();

  const [content, setContent] = useState<WorkspaceFileContent | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [htmlMode, setHtmlMode] = useState<'preview' | 'source'>('preview');
  const [isCopied, setIsCopied] = useState(false);

  // Reset states and fetch when file changes or sheet opens
  useEffect(() => {
    if (!isOpen || !file || file.isDirectory) {
      setContent(null);
      setError(null);
      setLoading(false);
      setHtmlMode('preview');
      setIsCopied(false);
      return;
    }

    let isCancelled = false;
    setLoading(true);
    setError(null);

    readWorkspaceFileContent(file.path, sessionId)
      .then((res) => {
        if (!isCancelled) {
          setContent(res);
          setLoading(false);
        }
      })
      .catch((err) => {
        if (!isCancelled) {
          const message =
            err instanceof Error
              ? err.message
              : String(err ?? 'Failed to load file');
          setError(message);
          setLoading(false);
        }
      });

    return () => {
      isCancelled = true;
    };
  }, [file, sessionId, isOpen, readWorkspaceFileContent]);

  const ext = useMemo(() => (file ? getFileExtension(file.name) : ''), [file]);
  const iconInfo = useMemo(
    () => (file ? getFileIconInfo(file.name) : null),
    [file],
  );
  const isHtml = ext === 'html' || ext === 'htm';
  const isMarkdown = ext === 'md' || ext === 'markdown';
  const isImage = iconInfo?.category === 'image';
  const language = useMemo(
    () => (file ? getLanguageFromFileName(file.name) : 'text'),
    [file],
  );

  const handleCopy = useCallback(async () => {
    if (!content?.content) return;
    try {
      await navigator.clipboard.writeText(content.content);
      setIsCopied(true);
      toast.success(
        t('agent.workspace.contentCopied', 'Content copied to clipboard'),
      );
      setTimeout(() => setIsCopied(false), 2000);
    } catch {
      toast.error('Failed to copy content to clipboard');
    }
  }, [content?.content, t]);

  const handleOpenDefault = useCallback(async () => {
    if (!file) return;
    await onOpenInDefaultApp(file.path);
  }, [file, onOpenInDefaultApp]);

  if (!file) return null;

  const Icon = iconInfo?.icon;
  const isLargeCodeFile =
    Boolean(content?.content) && (content?.content.length ?? 0) > 200 * 1024;

  return (
    <Sheet open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <SheetContent
        side="right"
        className="w-[600px] sm:max-w-2xl max-w-full flex flex-col p-0 gap-0 border-l border-border/40 shadow-xl"
      >
        {/* Header */}
        <SheetHeader className="border-b border-border/40 px-4 py-3 flex-shrink-0 flex-row items-center justify-between gap-2">
          <div className="flex items-center gap-2 min-w-0 flex-1">
            {Icon && (
              <Icon
                className={cn('h-4 w-4 flex-shrink-0', iconInfo?.className)}
              />
            )}
            <SheetTitle
              className="truncate text-sm font-medium"
              title={file.name}
            >
              {file.name}
            </SheetTitle>
            <SheetDescription className="sr-only">
              {file.name} preview
            </SheetDescription>
            {content?.size !== undefined && (
              <Badge variant="secondary" className="text-[10px] px-1.5 py-0">
                {formatFileSize(content.size)}
              </Badge>
            )}
          </div>

          <div className="flex items-center gap-1.5 mr-6 flex-shrink-0">
            {isHtml && !loading && !error && content && !content.isBinary && (
              <div className="flex items-center rounded-md border border-border/60 bg-muted/30 p-0.5 text-xs mr-1">
                <button
                  type="button"
                  onClick={() => setHtmlMode('preview')}
                  className={cn(
                    'flex items-center gap-1 rounded px-2 py-0.5 font-medium transition-colors cursor-pointer',
                    htmlMode === 'preview'
                      ? 'bg-background text-foreground shadow-xs'
                      : 'text-muted-foreground hover:text-foreground',
                  )}
                >
                  <Eye className="h-3 w-3" />
                  {t('agent.workspace.previewMode', 'Preview')}
                </button>
                <button
                  type="button"
                  onClick={() => setHtmlMode('source')}
                  className={cn(
                    'flex items-center gap-1 rounded px-2 py-0.5 font-medium transition-colors cursor-pointer',
                    htmlMode === 'source'
                      ? 'bg-background text-foreground shadow-xs'
                      : 'text-muted-foreground hover:text-foreground',
                  )}
                >
                  <Code className="h-3 w-3" />
                  {t('agent.workspace.sourceMode', 'Source')}
                </button>
              </div>
            )}

            {!isImage && content && !content.isBinary && (
              <Button
                variant="ghost"
                size="sm"
                className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground"
                onClick={handleCopy}
                title={t('agent.workspace.copyContent', 'Copy Content')}
                aria-label={t('agent.workspace.copyContent', 'Copy Content')}
              >
                {isCopied ? (
                  <Check className="h-3.5 w-3.5 text-emerald-500" />
                ) : (
                  <Copy className="h-3.5 w-3.5" />
                )}
              </Button>
            )}

            <Button
              variant="outline"
              size="sm"
              className="h-7 text-xs px-2"
              onClick={handleOpenDefault}
            >
              <ExternalLink className="h-3 w-3 mr-1" />
              {t('agent.workspace.openInDefaultApp', 'Open in Default App')}
            </Button>
          </div>
        </SheetHeader>

        {/* Content Body */}
        <div className="flex-1 overflow-hidden flex flex-col bg-background/50">
          {loading && (
            <div className="flex flex-1 items-center justify-center py-12">
              <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
            </div>
          )}

          {!loading && error && (
            <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
              <AlertTriangle className="h-8 w-8 text-destructive" />
              <p className="text-xs font-medium text-destructive max-w-md">
                {error}
              </p>
              <Button
                variant="outline"
                size="sm"
                className="text-xs"
                onClick={handleOpenDefault}
              >
                <ExternalLink className="h-3.5 w-3.5 mr-1.5" />
                {t('agent.workspace.openInDefaultApp', 'Open in Default App')}
              </Button>
            </div>
          )}

          {!loading && !error && content && (
            <>
              {/* Binary / Null-byte / Decode error fallback */}
              {content.isBinary && !isImage && (
                <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
                  <AlertTriangle className="h-8 w-8 text-amber-500" />
                  <p className="text-xs text-muted-foreground max-w-sm">
                    {t(
                      'agent.workspace.unsupportedEncoding',
                      'Binary or unsupported encoding. Please open with default application.',
                    )}
                  </p>
                  <Button
                    variant="outline"
                    size="sm"
                    className="text-xs"
                    onClick={handleOpenDefault}
                  >
                    <ExternalLink className="h-3.5 w-3.5 mr-1.5" />
                    {t(
                      'agent.workspace.openInDefaultApp',
                      'Open in Default App',
                    )}
                  </Button>
                </div>
              )}

              {/* Image Viewer */}
              {isImage && (
                <div className="flex flex-1 items-center justify-center overflow-auto p-4 bg-muted/10">
                  <img
                    src={`data:${content.mimeType};base64,${content.content}`}
                    alt={file.name}
                    className="max-h-full max-w-full rounded-md object-contain shadow-xs border border-border/30"
                  />
                </div>
              )}

              {/* HTML Viewer */}
              {isHtml && !content.isBinary && (
                <div className="flex-1 w-full h-full overflow-hidden">
                  {htmlMode === 'preview' ? (
                    <iframe
                      srcDoc={content.content}
                      sandbox="allow-scripts allow-modals"
                      className="w-full h-full border-0 bg-white dark:bg-zinc-950"
                      title={`Preview of ${file.name}`}
                      data-testid="html-preview-iframe"
                    />
                  ) : (
                    <div className="h-full overflow-auto p-4">
                      <CodeBlock className="language-html">
                        {content.content}
                      </CodeBlock>
                    </div>
                  )}
                </div>
              )}

              {/* Markdown Viewer */}
              {isMarkdown && !content.isBinary && (
                <div className="flex-1 overflow-auto p-6 prose dark:prose-invert max-w-none text-xs leading-relaxed">
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>
                    {content.content}
                  </ReactMarkdown>
                </div>
              )}

              {/* Code / Config / Text Viewer */}
              {!isHtml && !isMarkdown && !isImage && !content.isBinary && (
                <div className="flex-1 overflow-auto p-4">
                  {isLargeCodeFile ? (
                    <div>
                      <div className="mb-2 rounded bg-amber-500/10 border border-amber-500/20 px-2.5 py-1 text-[11px] text-amber-600 dark:text-amber-400">
                        {t(
                          'agent.workspace.syntaxHighlightDisabled',
                          'Large file (>200KB) - syntax highlighting disabled for performance',
                        )}
                      </div>
                      <pre className="font-mono text-xs whitespace-pre-wrap break-all text-foreground/90">
                        {content.content}
                      </pre>
                    </div>
                  ) : (
                    <CodeBlock className={`language-${language}`}>
                      {content.content}
                    </CodeBlock>
                  )}
                </div>
              )}
            </>
          )}
        </div>
      </SheetContent>
    </Sheet>
  );
};
