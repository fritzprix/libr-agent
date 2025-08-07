import { getLogger } from '@/lib/logger';
import { Message } from '@/models/chat';
import { useClipboard } from '@/hooks/useClipboard';
import React, { useEffect } from 'react';
import ReactMarkdown from 'react-markdown';
import { Copy, Check } from 'lucide-react';

const logger = getLogger('ContentBubble');

interface ContentBubbleProps {
  message: Message;
}

const ContentBubble: React.FC<ContentBubbleProps> = ({ message }) => {
  const { isStreaming, content } = message;
  const safeContent = typeof content === 'string' ? content : '';
  const { copied, copyToClipboard } = useClipboard();

  useEffect(() => {
    logger.info('message', { message });
  }, []);

  const handleCopy = async () => {
    try {
      await copyToClipboard(safeContent);
    } catch (err) {
      logger.error('Failed to copy content', err);
    }
  };

  if (!safeContent.trim()) {
    return null;
  }

  // 스트리밍 중일 때 불완전한 마크다운을 안전하게 처리
  const processStreamingContent = (text: string): string => {
    if (!isStreaming) return text;

    let processedText = text;

    // 불완전한 코드 블록 처리 (``` 로 시작했지만 닫히지 않은 경우)
    const codeBlockMatches = processedText.match(/```[^`]*$/);
    if (codeBlockMatches && !processedText.endsWith('```')) {
      // 마지막에 열려있는 코드 블록이 있다면 임시로 닫지 않고 그대로 둠
      // ReactMarkdown이 알아서 처리하도록 함
    }

    // 불완전한 링크 처리 [text](incomplete...
    processedText = processedText.replace(/\[([^\]]*)\]\([^)]*$/, '[$1]');

    // 불완전한 이미지 처리 ![alt](incomplete...
    processedText = processedText.replace(/!\[([^\]]*)\]\([^)]*$/, '![$1]');

    return processedText;
  };

  const processedContent = processStreamingContent(safeContent);

  return (
    <div className="group relative text-sm leading-relaxed">
      {/* Copy button - hover로 나타남 */}
      <button
        onClick={handleCopy}
        className="absolute top-2 right-2 flex items-center gap-1 px-2 py-1 bg-secondary hover:bg-secondary/80 text-secondary-foreground text-xs rounded transition-all opacity-0 group-hover:opacity-100"
        aria-label="Copy content"
      >
        {copied ? <Check size={12} /> : <Copy size={12} />}
        {copied ? 'Copied!' : 'Copy'}
      </button>
      
      <ReactMarkdown
        // 에러가 발생해도 앱이 크래시되지 않도록 처리
        skipHtml={false}
        // 불완전한 구문에 대해 관대하게 처리
        remarkPlugins={[]}
        rehypePlugins={[]}
        components={{
          // 에러가 발생한 경우 원본 텍스트를 그대로 표시
          p: ({ children, ...props }) => {
            try {
              return <p {...props}>{children}</p>;
            } catch {
              return <p {...props}>{String(children)}</p>;
            }
          },
          code: ({ children, className }) => {
            try {
              return <code className={className}>{children}</code>;
            } catch {
              return <code>{String(children)}</code>;
            }
          },
          pre: ({ children, ...props }) => {
            try {
              return <pre {...props}>{children}</pre>;
            } catch {
              return <pre {...props}>{String(children)}</pre>;
            }
          },
        }}
      >
        {processedContent}
      </ReactMarkdown>
    </div>
  );
};

export default ContentBubble;
