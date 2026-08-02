import React, { memo } from 'react';
import ReactMarkdown from 'react-markdown';
import { REMARK_PLUGINS, REHYPE_PLUGINS } from '../config/markdown';
import { useStreamMarkdownPreprocess } from '../hooks/useStreamMarkdownPreprocess';

// Memoized markdown text component to prevent re-renders when parent updates
export const MarkdownText = memo(
  ({
    content,
    components,
    isStreaming = false,
  }: {
    content: string;
    components: React.ComponentProps<typeof ReactMarkdown>['components'];
    isStreaming?: boolean;
  }) => {
    const displayContent = useStreamMarkdownPreprocess(content, isStreaming);

    return (
      <div className="relative text-sm leading-relaxed break-words font-sans">
        <ReactMarkdown
          skipHtml={false}
          remarkPlugins={REMARK_PLUGINS}
          rehypePlugins={REHYPE_PLUGINS}
          components={components}
        >
          {displayContent}
        </ReactMarkdown>
      </div>
    );
  },
);

MarkdownText.displayName = 'MarkdownText';
