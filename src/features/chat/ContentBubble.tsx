

import React from 'react';

interface ContentBubbleProps {
  content: string;
}

const ContentBubble: React.FC<ContentBubbleProps> = ({ content }) => {
  const formatContent = (content: string) => {
    // Simple markdown-like formatting for better readability
    return content.split('\n').map((line, index) => {
      // Handle code blocks
      if (line.startsWith('```')) {
        return (
          <div key={index} className="text-xs text-gray-400 font-mono">
            {line}
          </div>
        );
      }
      // Handle headers
      if (line.startsWith('# ')) {
        return (
          <div key={index} className="font-bold text-lg mt-3 mb-1">
            {line.substring(2)}
          </div>
        );
      }
      if (line.startsWith('## ')) {
        return (
          <div key={index} className="font-bold text-base mt-2 mb-1">
            {line.substring(3)}
          </div>
        );
      }
      // Handle bold text
      if (line.includes('**')) {
        const parts = line.split('**');
        return (
          <div key={index}>
            {parts.map((part, i) =>
              i % 2 === 1 ? <strong key={i}>{part}</strong> : part,
            )}
          </div>
        );
      }
      return <div key={index}>{line || '\u00A0'}</div>;
    });
  };

  return (
    <div className="text-sm leading-relaxed">
      {formatContent(content)}
    </div>
  );
};

export default ContentBubble;

