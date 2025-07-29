import React from 'react';

interface AttachmentBubbleProps {
  attachments: {
    name: string;
    content: string;
  }[];
}

const AttachmentBubble: React.FC<AttachmentBubbleProps> = ({ attachments }) => {
  return (
    <div className="mt-4 p-3 bg-popover rounded-lg border border-border">
      <div className="flex items-center gap-2 mb-2">
        <span className="text-sm">📎</span>
        <span className="text-sm font-medium">
          {attachments.length} file{attachments.length > 1 ? 's' : ''} attached
        </span>
      </div>
      <div className="flex flex-wrap gap-2">
        {attachments.map((attachment, index) => (
          <div
            key={index}
            className="inline-flex items-center gap-2 px-3 py-1 bg-muted rounded-full text-xs"
          >
            <span className="text-success">📄</span>
            <span className="truncate max-w-32">{attachment.name}</span>
          </div>
        ))}
      </div>
    </div>
  );
};

export default AttachmentBubble;
