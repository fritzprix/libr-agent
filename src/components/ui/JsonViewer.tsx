import React, { useState } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';

export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonObject
  | JsonArray;
export type JsonObject = { [key: string]: JsonValue };
export type JsonArray = JsonValue[];

interface JsonViewerProps {
  data: JsonValue;
  level?: number;
}

export const JsonViewer: React.FC<JsonViewerProps> = ({ data, level = 0 }) => {
  const [collapsed, setCollapsed] = useState(level > 2);
  const indent = '  '.repeat(level);

  const renderValue = (value: JsonValue, isLastItem?: boolean) => {
    const comma = !isLastItem ? ',' : '';

    if (value === null) {
      return <span className="text-muted-foreground">null{comma}</span>;
    }

    if (typeof value === 'boolean') {
      return (
        <span className="text-violet-600 dark:text-violet-400">
          {value.toString()}
          {comma}
        </span>
      );
    }

    if (typeof value === 'number') {
      return (
        <span className="text-blue-600 dark:text-blue-400">
          {value}
          {comma}
        </span>
      );
    }

    if (typeof value === 'string') {
      return (
        <span className="text-emerald-600 dark:text-emerald-400">
          &quot;{value}&quot;{comma}
        </span>
      );
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
                  <span className="text-muted-foreground">{indent} </span>
                  {renderValue(item, index === value.length - 1)}
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
        return (
          <span className="text-muted-foreground">
            {`{}`}
            {comma}
          </span>
        );
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
                  <span className="text-muted-foreground">{indent} </span>
                  <span className="text-orange-600 dark:text-orange-400">
                    &quot;{objKey}&quot;
                  </span>
                  <span className="text-muted-foreground">: </span>
                  {renderValue(value[objKey], index === keys.length - 1)}
                </div>
              ))}
            </div>
          )}
          <div className="font-mono text-sm">
            <span className="text-muted-foreground">{indent}</span>
            <span className="text-muted-foreground">
              {'}'}
              {comma}
            </span>
          </div>
        </div>
      );
    }

    return (
      <span className="text-muted-foreground">
        {String(value)}
        {comma}
      </span>
    );
  };

  return <div>{renderValue(data)}</div>;
};
