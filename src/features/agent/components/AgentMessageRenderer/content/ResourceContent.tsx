import React, { useMemo, useEffect, useRef } from 'react';
import {
  basicComponentLibrary,
  UIResourceRenderer,
  UIActionResult,
  remoteButtonDefinition,
  remoteTextDefinition,
  remoteCardDefinition,
  remoteImageDefinition,
  remoteStackDefinition,
} from '@mcp-ui/client';

interface ResourceContentProps {
  resource: {
    uri: string;
    mimeType: string;
    text?: string;
    blob?: string;
    _meta?: Record<string, unknown>;
  };
  expandResources?: boolean;
  onUIAction: (result: UIActionResult) => Promise<void | { status: string; tool?: string; intent?: string; message?: string }>;
}

export const ResourceContent: React.FC<ResourceContentProps> = ({
  resource,
  expandResources = false,
  onUIAction,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);

  // When resources are allowed to expand, watch size changes and scroll them into view
  useEffect(() => {
    if (!expandResources || !containerRef.current) return;

    const el = containerRef.current;
    let lastHeight = el.getBoundingClientRect().height;

    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const height = entry.contentRect.height;
        if (height > lastHeight) {
          // Ensure the newly expanded content is visible in the scrollable container
          try {
            el.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
          } catch {
            // ignore
          }
        }
        lastHeight = height;
      }
    });

    ro.observe(el);
    return () => ro.disconnect();
  }, [expandResources]);

  // Memoize renderer props to keep identity stable across re-renders
  const remoteDomProps = useMemo(
    () => ({
      library: basicComponentLibrary,
      remoteElements: [
        remoteButtonDefinition,
        remoteTextDefinition,
        remoteCardDefinition,
        remoteImageDefinition,
        remoteStackDefinition,
      ],
    }),
    [],
  );

  const supportedContentTypes = useMemo(
    () => ['rawHtml', 'externalUrl', 'remoteDom'] as const,
    [],
  );

  // Memoize mutable supported content types array to prevent re-creation on every render
  const mutableSupportedContentTypes = useMemo(
    () => [...supportedContentTypes],
    [supportedContentTypes],
  );

  // Memoize htmlProps to prevent re-creation on every render
  const htmlProps = useMemo(
    () => ({
      style: { height: 'auto', maxHeight: 'unset' },
      iframeProps: {
        className: 'h-auto min-h-96 max-h-none',
      },
    }),
    [],
  );

  return (
    <div
      ref={containerRef}
      className={expandResources ? 'w-full overflow-visible min-h-96' : ''}
    >
      <UIResourceRenderer
        remoteDomProps={remoteDomProps}
        onUIAction={onUIAction}
        supportedContentTypes={mutableSupportedContentTypes}
        htmlProps={htmlProps}
        resource={resource}
      />
    </div>
  );
};
