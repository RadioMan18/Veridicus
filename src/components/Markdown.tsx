import type { ReactNode } from 'react';

/**
 * Small markdown renderer for model output and research notes.
 * Deliberately dependency-free: headings, lists, quotes, code and inline emphasis.
 * Technical output (errors, command input) stays monospace in the caller.
 */

function renderInline(text: string, keyPrefix: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  const pattern = /(\*\*[^*]+\*\*|\*[^*]+\*|`[^`]+`)/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  let index = 0;

  while ((match = pattern.exec(text)) !== null) {
    if (match.index > lastIndex) {
      nodes.push(text.slice(lastIndex, match.index));
    }
    const token = match[0];
    const key = `${keyPrefix}-${index++}`;
    if (token.startsWith('**')) {
      nodes.push(
        <strong key={key} className="font-semibold text-aether-text">
          {token.slice(2, -2)}
        </strong>,
      );
    } else if (token.startsWith('`')) {
      nodes.push(
        <code key={key} className="rounded bg-aether-bg px-1 py-0.5 font-mono text-[0.85em]">
          {token.slice(1, -1)}
        </code>,
      );
    } else {
      nodes.push(<em key={key}>{token.slice(1, -1)}</em>);
    }
    lastIndex = match.index + token.length;
  }

  if (lastIndex < text.length) {
    nodes.push(text.slice(lastIndex));
  }
  return nodes;
}

const HEADING_SIZES = ['text-2xl', 'text-xl', 'text-lg', 'text-base', 'text-sm', 'text-sm'];

export default function Markdown({
  content,
  className = '',
}: {
  content: string;
  className?: string;
}) {
  const lines = content.replace(/\r\n/g, '\n').split('\n');
  const blocks: ReactNode[] = [];
  let index = 0;
  let key = 0;

  while (index < lines.length) {
    const trimmed = lines[index].trim();

    if (!trimmed) {
      index += 1;
      continue;
    }

    if (trimmed.startsWith('```')) {
      const code: string[] = [];
      index += 1;
      while (index < lines.length && !lines[index].trim().startsWith('```')) {
        code.push(lines[index]);
        index += 1;
      }
      index += 1;
      blocks.push(
        <pre
          key={key++}
          className="my-3 overflow-x-auto rounded-lg bg-aether-bg p-3 font-mono text-xs leading-relaxed"
        >
          {code.join('\n')}
        </pre>,
      );
      continue;
    }

    const heading = /^(#{1,6})\s+(.*)$/.exec(trimmed);
    if (heading) {
      const level = heading[1].length;
      blocks.push(
        <p
          key={key++}
          className={`mb-2 mt-5 font-semibold text-aether-text ${HEADING_SIZES[level - 1]}`}
        >
          {renderInline(heading[2], `h-${key}`)}
        </p>,
      );
      index += 1;
      continue;
    }

    if (/^(-{3,}|\*{3,}|_{3,})$/.test(trimmed)) {
      blocks.push(<hr key={key++} className="my-4 border-aether-border" />);
      index += 1;
      continue;
    }

    if (/^[-*•]\s+/.test(trimmed)) {
      const items: string[] = [];
      while (index < lines.length && /^\s*[-*•]\s+/.test(lines[index])) {
        items.push(lines[index].replace(/^\s*[-*•]\s+/, ''));
        index += 1;
      }
      blocks.push(
        <ul key={key++} className="my-2 list-disc space-y-1 pl-5 text-sm leading-relaxed">
          {items.map((item, itemIndex) => (
            <li key={itemIndex}>{renderInline(item, `li-${itemIndex}`)}</li>
          ))}
        </ul>,
      );
      continue;
    }

    if (/^\d+\.\s+/.test(trimmed)) {
      const items: string[] = [];
      while (index < lines.length && /^\s*\d+\.\s+/.test(lines[index])) {
        items.push(lines[index].replace(/^\s*\d+\.\s+/, ''));
        index += 1;
      }
      blocks.push(
        <ol key={key++} className="my-2 list-decimal space-y-1 pl-5 text-sm leading-relaxed">
          {items.map((item, itemIndex) => (
            <li key={itemIndex}>{renderInline(item, `oli-${itemIndex}`)}</li>
          ))}
        </ol>,
      );
      continue;
    }

    if (trimmed.startsWith('>')) {
      const quote: string[] = [];
      while (index < lines.length && lines[index].trim().startsWith('>')) {
        quote.push(lines[index].trim().replace(/^>\s?/, ''));
        index += 1;
      }
      blocks.push(
        <blockquote
          key={key++}
          className="my-3 border-l-2 border-aether-border pl-3 text-sm italic text-aether-text-muted"
        >
          {renderInline(quote.join(' '), 'quote')}
        </blockquote>,
      );
      continue;
    }

    const paragraph: string[] = [];
    while (
      index < lines.length &&
      lines[index].trim() &&
      !/^(#{1,6}\s|[-*•]\s|\d+\.\s|>|```)/.test(lines[index].trim())
    ) {
      paragraph.push(lines[index].trim());
      index += 1;
    }

    if (paragraph.length === 0) {
      index += 1;
      continue;
    }

    blocks.push(
      <p key={key++} className="my-2 text-sm leading-relaxed text-aether-text">
        {renderInline(paragraph.join(' '), 'p')}
      </p>,
    );
  }

  return <div className={`max-w-[72ch] ${className}`}>{blocks}</div>;
}
