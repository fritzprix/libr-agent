import React, { useMemo } from 'react';
import ReactMarkdown from 'react-markdown';
import 'katex/dist/katex.min.css';
import { Copy, Check } from 'lucide-react';
import { useClipboard } from '@/hooks/useClipboard';
import { getLogger } from '@/lib/logger';
import { REMARK_PLUGINS, REHYPE_PLUGINS } from '../markdown/plugins';
import { CodeBlock } from '../markdown/CodeBlock';
import { STATIC_MARKDOWN_COMPONENTS } from '../markdown/MarkdownComponents';

const logger = getLogger('AgentMessageRenderer:TextContent');

interface TextContentProps {
  text: string;
  isDark: boolean;
}

export const TextContent: React.FC<TextContentProps> = ({ text, isDark }) => {
  const { copied, copyToClipboard } = useClipboard();

  // Memoize markdown components to include dynamic isDark prop
  const markdownComponents = useMemo(
    () => ({
      ...STATIC_MARKDOWN_COMPONENTS,
      code: ({
        children,
        className,
        node, // Destructure node to exclude it from props passed to CodeBlock
        ...props
      }: React.ComponentPropsWithoutRef<'code'> & {
        inline?: boolean;
        node?: unknown;
      }) => {
        void node;
        return (
          <CodeBlock isDark={isDark} className={className} {...props}>
            {children}
          </CodeBlock>
        );
      },
    }),
    [isDark],
  );

  return (
    <div className="group relative text-sm leading-relaxed break-words">
      {/* Copy button for individual text */}
      <button
        onClick={async () => {
          try {
            await copyToClipboard(text);
          } catch (err) {
            logger.error('Failed to copy text content', err);
          }
        }}
        className="absolute top-2 right-2 flex items-center gap-1 px-2 py-1 bg-secondary hover:bg-secondary/80 text-secondary-foreground text-xs rounded transition-all opacity-0 group-hover:opacity-100 z-10"
        aria-label="Copy text content"
      >
        {copied ? <Check size={12} /> : <Copy size={12} />}
        {copied ? 'Copied!' : 'Copy'}
      </button>

      <ReactMarkdown
        skipHtml={false}
        remarkPlugins={REMARK_PLUGINS}
        rehypePlugins={REHYPE_PLUGINS}
        components={markdownComponents}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
};
