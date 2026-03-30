import {
  ChevronRight,
  ChevronDown,
  File,
  Folder,
  FolderOpen,
  RefreshCw,
} from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import type { FileNode } from './types';

interface FileTreeNodeProps {
  node: FileNode;
  depth?: number;
  onToggle: (node: FileNode) => void;
  onOpen: (node: FileNode) => void;
}

export const FileTreeNode = ({
  node,
  depth = 0,
  onToggle,
  onOpen,
}: FileTreeNodeProps) => {
  const Icon = node.isDirectory
    ? node.isExpanded
      ? FolderOpen
      : Folder
    : File;

  return (
    <div className="select-none">
      <div
        className="group flex items-center gap-1.5 px-2 py-1.5 text-foreground/85 transition-colors hover:bg-foreground/[0.03]"
        style={{ paddingLeft: `${8 + depth * 16}px` }}
        onClick={() => {
          // Keep mouse click behavior for padding area
          if (node.isDirectory) {
            onToggle(node);
          } else {
            onOpen(node);
          }
        }}
      >
        {node.isDirectory ? (
          <button
            type="button"
            className="flex h-4 w-4 items-center justify-center rounded-sm text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            onClick={(e) => {
              e.stopPropagation();
              onToggle(node);
            }}
            aria-label={node.isExpanded ? 'Collapse' : 'Expand'}
            aria-expanded={node.isExpanded}
          >
            {node.isLoading ? (
              <RefreshCw className="w-3 h-3 animate-spin" />
            ) : node.isExpanded ? (
              <ChevronDown className="w-3 h-3" />
            ) : (
              <ChevronRight className="w-3 h-3" />
            )}
          </button>
        ) : (
          <div className="h-4 w-4" /> // Spacer
        )}

        <div
          role="button"
          tabIndex={0}
          className="flex min-w-0 flex-1 cursor-pointer items-center gap-1.5 rounded-sm px-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          onClick={(e) => {
            e.stopPropagation();
            if (node.isDirectory) {
              onToggle(node);
            } else {
              onOpen(node);
            }
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              e.stopPropagation();
              if (node.isDirectory) {
                onToggle(node);
              } else {
                onOpen(node);
              }
            }
          }}
          aria-expanded={node.isDirectory ? node.isExpanded : undefined}
        >
          <Icon className="h-4 w-4 flex-shrink-0 text-muted-foreground" />

          <span className="flex-1 truncate text-xs" title={node.name}>
            {node.name}
          </span>

          {node.isDirectory && (
            <Badge
              variant="secondary"
              className="px-1 text-[10px] opacity-0 transition-opacity group-hover:opacity-100"
            >
              {node.children?.length || 0}
            </Badge>
          )}
        </div>
      </div>

      {node.isExpanded && node.children && (
        <div>
          {node.children.map((child) => (
            <FileTreeNode
              key={child.id}
              node={child}
              depth={depth + 1}
              onToggle={onToggle}
              onOpen={onOpen}
            />
          ))}
        </div>
      )}
    </div>
  );
};
