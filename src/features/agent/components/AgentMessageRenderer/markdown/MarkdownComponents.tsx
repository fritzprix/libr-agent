import React from 'react';
import ReactMarkdown from 'react-markdown';

// Helper type to avoid implicit any in markdown components
export type MarkdownReflessProps<T extends React.ElementType> =
  React.ComponentPropsWithoutRef<T> & {
    node?: unknown;
  };

// Define static markdown components outside to prevent re-creation on every render
export const STATIC_MARKDOWN_COMPONENTS: Omit<
  React.ComponentProps<typeof ReactMarkdown>['components'],
  'code'
> = {
  p: ({ children, ...props }: MarkdownReflessProps<'p'>) => (
    <p className="mb-2 last:mb-0" {...props}>
      {children}
    </p>
  ),
  pre: ({ children, ...props }: MarkdownReflessProps<'pre'>) => (
    <pre
      className="overflow-x-auto bg-muted rounded-lg p-4 my-3 border border-border max-w-full"
      {...props}
    >
      {children}
    </pre>
  ),
  table: ({ children, ...props }: MarkdownReflessProps<'table'>) => (
    <div className="overflow-x-auto w-full max-w-full my-4 border rounded-lg">
      <table className="w-full text-sm text-left" {...props}>
        {children}
      </table>
    </div>
  ),
  thead: ({ children, ...props }: MarkdownReflessProps<'thead'>) => (
    <thead className="bg-muted/50 text-muted-foreground" {...props}>
      {children}
    </thead>
  ),
  tbody: ({ children, ...props }: MarkdownReflessProps<'tbody'>) => (
    <tbody className="divide-y divide-border" {...props}>
      {children}
    </tbody>
  ),
  tr: ({ children, ...props }: MarkdownReflessProps<'tr'>) => (
    <tr
      className="border-b border-border last:border-0 hover:bg-muted/30 transition-colors"
      {...props}
    >
      {children}
    </tr>
  ),
  th: ({ children, ...props }: MarkdownReflessProps<'th'>) => (
    <th className="px-4 py-3 font-medium" {...props}>
      {children}
    </th>
  ),
  td: ({ children, ...props }: MarkdownReflessProps<'td'>) => (
    <td className="px-4 py-3" {...props}>
      {children}
    </td>
  ),
  h1: ({ children, ...props }: MarkdownReflessProps<'h1'>) => (
    <h1 className="text-2xl font-bold mb-3 mt-4" {...props}>
      {children}
    </h1>
  ),
  h2: ({ children, ...props }: MarkdownReflessProps<'h2'>) => (
    <h2 className="text-xl font-bold mb-2 mt-3" {...props}>
      {children}
    </h2>
  ),
  h3: ({ children, ...props }: MarkdownReflessProps<'h3'>) => (
    <h3 className="text-lg font-semibold mb-2 mt-2" {...props}>
      {children}
    </h3>
  ),
  ul: ({ children, ...props }: MarkdownReflessProps<'ul'>) => (
    <ul className="list-disc list-inside mb-2 space-y-1" {...props}>
      {children}
    </ul>
  ),
  ol: ({ children, ...props }: MarkdownReflessProps<'ol'>) => (
    <ol className="list-decimal list-inside mb-2 space-y-1" {...props}>
      {children}
    </ol>
  ),
  li: ({ children, ...props }: MarkdownReflessProps<'li'>) => (
    <li className="ml-2" {...props}>
      {children}
    </li>
  ),
  blockquote: ({ children, ...props }: MarkdownReflessProps<'blockquote'>) => (
    <blockquote
      className="border-l-4 border-primary pl-4 italic my-2 text-muted-foreground"
      {...props}
    >
      {children}
    </blockquote>
  ),
  strong: ({ children, ...props }: MarkdownReflessProps<'strong'>) => (
    <strong className="font-bold" {...props}>
      {children}
    </strong>
  ),
  em: ({ children, ...props }: MarkdownReflessProps<'em'>) => (
    <em className="italic" {...props}>
      {children}
    </em>
  ),
  a: ({ children, href, ...props }: MarkdownReflessProps<'a'>) => (
    <a
      href={href}
      className="text-primary hover:text-primary/90 underline font-medium"
      target="_blank"
      rel="noopener noreferrer"
      {...props}
    >
      {children}
    </a>
  ),
};
