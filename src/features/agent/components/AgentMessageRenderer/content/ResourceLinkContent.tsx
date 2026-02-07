import React from 'react';
import { useRustBackend } from '@/hooks/use-rust-backend';

interface ResourceLinkContentProps {
  link: {
    uri: string;
    name: string;
    description?: string;
  };
}

export const ResourceLinkContent: React.FC<ResourceLinkContentProps> = ({
  link,
}) => {
  const { openExternalUrl } = useRustBackend();

  const handleLinkClick = async (e: React.MouseEvent, url: string) => {
    e.preventDefault();

    try {
      await openExternalUrl(url);
    } catch {
      // Fallback for browser environment
      if (typeof window !== 'undefined') {
        window.open(url, '_blank', 'noopener,noreferrer');
      }
    }
  };

  return (
    <div className="p-2 border rounded-lg bg-muted">
      <a
        href={link.uri}
        onClick={(e) => handleLinkClick(e, link.uri)}
        className="text-primary hover:text-primary/90 underline"
      >
        {link.name}
      </a>
      {link.description && (
        <div className="text-sm text-muted-foreground mt-1">
          {link.description}
        </div>
      )}
    </div>
  );
};
