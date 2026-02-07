import React, { memo } from 'react';
import { Highlight, themes } from 'prism-react-renderer';

// Extract CodeBlock component to allow injecting isDark prop
// Memoized to prevent expensive syntax highlighting re-runs during text streaming
export const CodeBlock = memo(
  ({
    children,
    className,
    isDark,
    ...props
  }: React.ComponentPropsWithoutRef<'code'> & {
    inline?: boolean;
    node?: unknown;
    isDark?: boolean;
  }) => {
    // Distinguish inline code vs block code
    // ReactMarkdown passes className="language-xxx" for code blocks
    const match = /language-(\w+)/.exec(className || '');
    const language = match ? match[1] : '';

    if (!language) {
      // Inline code
      return (
        <code
          className="px-1.5 py-0.5 bg-muted rounded text-sm font-mono border border-border break-all"
          {...props}
        >
          {children}
        </code>
      );
    }

    // Block code with syntax highlighting
    const code = String(children).replace(/\n$/, '');

    return (
      <Highlight
        theme={isDark ? themes.oneDark : themes.oneLight}
        code={code}
        language={language}
      >
        {({
          className: highlightClassName,
          style,
          tokens,
          getLineProps,
          getTokenProps,
        }) => (
          <code
            className={`${highlightClassName} block font-mono text-sm`}
            style={style}
          >
            {tokens.map((line, i) => (
              <div key={i} {...getLineProps({ line })}>
                {line.map((token, key) => (
                  <span key={key} {...getTokenProps({ token })} />
                ))}
              </div>
            ))}
          </code>
        )}
      </Highlight>
    );
  },
);

CodeBlock.displayName = 'CodeBlock';
