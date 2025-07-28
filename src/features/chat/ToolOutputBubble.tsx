import React, { useState } from 'react';
import { ChevronDown, ChevronRight, Copy, Check } from 'lucide-react';

interface ToolOutputBubbleProps {
  content: string;
  defaultExpanded?: boolean;
}

type JsonValue = string | number | boolean | null | JsonObject | JsonArray;
type JsonObject = { [key: string]: JsonValue };
type JsonArray = JsonValue[];

interface JsonViewerProps {
  data: JsonValue;
  level?: number;
}

const JsonViewer: React.FC<JsonViewerProps> = ({ data, level = 0 }) => {
  const [collapsed, setCollapsed] = useState(level > 2);
  const indent = '  '.repeat(level);

  const renderValue = (value: JsonValue, key?: string, isLastItem?: boolean) => {
    const comma = !isLastItem ? ',' : '';

    if (value === null) {
      return <span className="text-muted-foreground">null{comma}</span>;
    }

    if (typeof value === 'boolean') {
      return <span className="text-violet-600 dark:text-violet-400">{value.toString()}{comma}</span>;
    }

    if (typeof value === 'number') {
      return <span className="text-blue-600 dark:text-blue-400">{value}{comma}</span>;
    }

    if (typeof value === 'string') {
      return <span className="text-emerald-600 dark:text-emerald-400">&quot;{value}&quot;{comma}</span>;
    }

    if (Array.isArray(value)) {
      if (value.length === 0) {
        return <span className="text-muted-foreground">[]</span>;
      }

      return (
        <div>
          <button
            onClick={() => setCollapsed(!collapsed)}
            className="inline-flex items-center text-muted-foreground hover:text-foreground transition-colors"
          >
            {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
            <span className="ml-1">[</span>
          </button>
          <span className="text-xs text-muted-foreground ml-1">
            {value.length} item{value.length !== 1 ? 's' : ''}
          </span>
          {!collapsed && (
            <div className="ml-4">
              {value.map((item, index) => (
                <div key={index} className="font-mono text-sm">
                  <span className="text-muted-foreground">{indent}  </span>
                  {renderValue(item, undefined, index === value.length - 1)}
                </div>
              ))}
            </div>
          )}
          <div className="font-mono text-sm">
            <span className="text-muted-foreground">{indent}</span>
            <span className="text-muted-foreground">]{comma}</span>
          </div>
        </div>
      );
    }

    if (typeof value === 'object') {
      const keys = Object.keys(value);
      if (keys.length === 0) {
        return <span className="text-muted-foreground">{`{}`}{comma}</span>;
      }

      return (
        <div>
          <button
            onClick={() => setCollapsed(!collapsed)}
            className="inline-flex items-center text-muted-foreground hover:text-foreground transition-colors"
          >
            {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
            <span className="ml-1">{'{'}</span>
          </button>
          <span className="text-xs text-muted-foreground ml-1">
            {keys.length} key{keys.length !== 1 ? 's' : ''}
          </span>
          {!collapsed && (
            <div className="ml-4">
              {keys.map((objKey, index) => (
                <div key={objKey} className="font-mono text-sm">
                  <span className="text-muted-foreground">{indent}  </span>
                  <span className="text-orange-600 dark:text-orange-400">&quot;{objKey}&quot;</span>
                  <span className="text-muted-foreground">: </span>
                  {renderValue(value[objKey], objKey, index === keys.length - 1)}
                </div>
              ))}
            </div>
          )}
          <div className="font-mono text-sm">
            <span className="text-muted-foreground">{indent}</span>
            <span className="text-muted-foreground">{'}'}{comma}</span>
          </div>
        </div>
      );
    }

    return <span className="text-muted-foreground">{String(value)}{comma}</span>;
  };

  return <div>{renderValue(data)}</div>;
};

export const ToolOutputBubble: React.FC<ToolOutputBubbleProps> = ({ content, defaultExpanded = false }) => {
  const [copied, setCopied] = useState(false);
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);

  const parsedContent = (() => {
    try {
      return JSON.parse(content);
    } catch {
      return null;
    }
  })();

  const copyToClipboard = async () => {
    try {
      await navigator.clipboard.writeText(parsedContent ? JSON.stringify(parsedContent, null, 2) : content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy text: ', err);
    }
  };

  const isJson = parsedContent !== null;

  return (
    <div className="mt-4 bg-background rounded-lg border border-border overflow-hidden shadow-lg">
      <div className="px-4 py-3 bg-muted border-b border-border flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex gap-1.5">
            <div className="w-3 h-3 bg-red-500 rounded-full"></div>
            <div className="w-3 h-3 bg-yellow-500 rounded-full"></div>
            <div className="w-3 h-3 bg-green-500 rounded-full"></div>
          </div>
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="flex items-center gap-2 text-muted-foreground hover:text-foreground transition-colors"
          >
            {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
            <span className="font-mono text-sm flex items-center gap-2">
              Tool Output
              {isJson && (
                <span className="px-2 py-1 bg-primary text-primary-foreground text-xs rounded-full">
                  JSON
                </span>
              )}
            </span>
          </button>
        </div>
        <button
          onClick={copyToClipboard}
          className="flex items-center gap-2 px-3 py-1.5 bg-secondary hover:bg-secondary/80 text-secondary-foreground text-xs rounded transition-colors"
        >
          {copied ? <Check size={12} /> : <Copy size={12} />}
          {copied ? 'Copied!' : 'Copy'}
        </button>
      </div>

      {!isExpanded && (
        <div className="px-4 py-3 bg-background/50 text-muted-foreground text-sm">
          {isJson ? (
            <span>
              {Array.isArray(parsedContent)
                ? `Array with ${parsedContent.length} items`
                : typeof parsedContent === 'object' && parsedContent !== null
                  ? `Object with ${Object.keys(parsedContent).length} keys`
                  : `${typeof parsedContent} value`
              }
            </span>
          ) : (
            <span>{content.length} characters</span>
          )}
        </div>
      )}

      {isExpanded && (
        <div className="p-4 max-h-96 overflow-auto bg-background">
          {isJson ? (
            <div className="text-sm">
              <JsonViewer data={parsedContent} />
            </div>
          ) : (
            <pre className="text-sm text-foreground font-mono whitespace-pre-wrap break-words">
              {content}
            </pre>
          )}
        </div>
      )}
    </div>
  );
};


export default ToolOutputBubble;