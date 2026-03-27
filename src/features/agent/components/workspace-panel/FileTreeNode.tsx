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
        className="flex items-center gap-1 px-2 py-1 hover:bg-muted/50 group"
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
            className="w-4 h-4 flex items-center justify-center focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-sm"
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
          <div className="w-4 h-4" /> // Spacer
        )}

        <div
          role="button"
          tabIndex={0}
          className="flex-1 flex items-center gap-1 min-w-0 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-sm px-1"
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
          <Icon className="w-4 h-4 flex-shrink-0" />

          <span className="text-xs truncate flex-1" title={node.name}>
            {node.name}
          </span>

          {node.isDirectory && (
            <Badge
              variant="secondary"
              className="text-xs px-1 opacity-0 group-hover:opacity-100 group-focus-visible:opacity-100"
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
