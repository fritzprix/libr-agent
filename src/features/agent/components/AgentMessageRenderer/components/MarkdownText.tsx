import React, { memo, useCallback } from 'react';
import ReactMarkdown from 'react-markdown';
import { Copy, Check } from 'lucide-react';
import { useClipboard } from '@/hooks/useClipboard';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { REMARK_PLUGINS, REHYPE_PLUGINS } from '../config/markdown';

const logger = getLogger('AgentMessageRenderer');

// Memoized markdown text component to prevent re-renders when parent updates
export const MarkdownText = memo(
  ({
    content,
    components,
  }: {
    content: string;
    components: React.ComponentProps<typeof ReactMarkdown>['components'];
  }) => {
    const { copied, copyToClipboard } = useClipboard();

    const handleCopy = useCallback(async () => {
      try {
        await copyToClipboard(content);
      } catch (err) {
        logger.error('Failed to copy text content', err);
        toast.error('Failed to copy content to clipboard');
      }
    }, [content, copyToClipboard]);

    return (
      <div className="group relative text-sm leading-relaxed break-words">
        {/* Copy button for individual text */}
        <button
          onClick={handleCopy}
          className="absolute top-2 right-2 flex items-center gap-1 px-2 py-1 bg-secondary hover:bg-secondary/80 text-secondary-foreground text-xs rounded transition-all opacity-0 group-hover:opacity-100 focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none z-10"
          aria-label="Copy text content"
        >
          {copied ? <Check size={12} /> : <Copy size={12} />}
          {copied ? 'Copied!' : 'Copy'}
        </button>

        <ReactMarkdown
          skipHtml={false}
          remarkPlugins={REMARK_PLUGINS}
          rehypePlugins={REHYPE_PLUGINS}
          components={components}
        >
          {content}
        </ReactMarkdown>
      </div>
    );
  },
);

MarkdownText.displayName = 'MarkdownText';
