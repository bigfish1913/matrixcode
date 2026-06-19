import React, { useState } from 'react';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark, oneLight } from 'react-syntax-highlighter/dist/esm/styles/prism';

interface CodeBlockProps {
  code: string;
  language: string;
  isDarkMode?: boolean;
  maxHeight?: number;
  showLineNumbers?: boolean;
  fileName?: string;
}

// Language display names
const LANGUAGE_NAMES: Record<string, string> = {
  js: 'JavaScript',
  jsx: 'React JSX',
  ts: 'TypeScript',
  tsx: 'React TSX',
  py: 'Python',
  rs: 'Rust',
  go: 'Go',
  java: 'Java',
  cpp: 'C++',
  c: 'C',
  rb: 'Ruby',
  php: 'PHP',
  swift: 'Swift',
  kt: 'Kotlin',
  scala: 'Scala',
  sql: 'SQL',
  json: 'JSON',
  yaml: 'YAML',
  xml: 'XML',
  html: 'HTML',
  css: 'CSS',
  scss: 'SCSS',
  sass: 'Sass',
  less: 'Less',
  md: 'Markdown',
  sh: 'Shell',
  bash: 'Bash',
  zsh: 'Zsh',
  dockerfile: 'Dockerfile',
  toml: 'TOML',
  ini: 'INI',
  diff: 'Diff',
  graphql: 'GraphQL',
  vue: 'Vue',
  svelte: 'Svelte',
  astro: 'Astro',
  default: 'Code',
};

// Get language display name
function getLanguageName(lang: string): string {
  return LANGUAGE_NAMES[lang.toLowerCase()] || LANGUAGE_NAMES.default;
}

// Copy button component
function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <button
      onClick={handleCopy}
      className={`px-2 py-1 rounded text-xs transition-colors ${
        copied
          ? 'bg-green-500/20 text-green-500'
          : 'bg-muted/50 text-muted-foreground hover:bg-muted hover:text-foreground'
      }`}
    >
      {copied ? '✓ Copied' : '📋 Copy'}
    </button>
  );
}

export function CodeBlock({
  code,
  language,
  isDarkMode = true,
  maxHeight = 300,
  showLineNumbers = false,
  fileName,
}: CodeBlockProps) {
  const [expanded, setExpanded] = useState(false);
  const [showLineNumbersState, setShowLineNumbersState] = useState(showLineNumbers);

  // Line count
  const lineCount = code.split('\n').length;
  const needsCollapse = lineCount > 10 || code.length > 500;

  // Current height
  const currentMaxHeight = expanded ? undefined : maxHeight;

  return (
    <div className="rounded-lg border overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 bg-muted/50 border-b">
        {/* Language and file info */}
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-muted-foreground">
            {getLanguageName(language)}
          </span>
          {fileName && (
            <span className="text-xs text-muted-foreground opacity-70">
              ({fileName})
            </span>
          )}
          <span className="text-xs text-muted-foreground opacity-50">
            {lineCount} lines
          </span>
        </div>

        {/* Action buttons */}
        <div className="flex items-center gap-2">
          {/* Line numbers toggle */}
          <button
            onClick={() => setShowLineNumbersState(!showLineNumbersState)}
            className={`px-2 py-1 rounded text-xs transition-colors ${
              showLineNumbersState
                ? 'bg-primary/20 text-primary'
                : 'bg-muted/50 text-muted-foreground hover:bg-muted'
            }`}
            title="Toggle line numbers"
          >
            #
          </button>

          {/* Expand/collapse toggle */}
          {needsCollapse && (
            <button
              onClick={() => setExpanded(!expanded)}
              className="px-2 py-1 rounded text-xs bg-muted/50 text-muted-foreground hover:bg-muted transition-colors"
            >
              {expanded ? '⬇ Collapse' : '⬆ Expand'}
            </button>
          )}

          {/* Copy button */}
          <CopyButton text={code} />
        </div>
      </div>

      {/* Code content */}
      <div
        className={`overflow-auto transition-all ${expanded ? '' : `max-h-[${maxHeight}px]`}`}
        style={{ maxHeight: currentMaxHeight }}
      >
        <SyntaxHighlighter
          language={language}
          style={isDarkMode ? oneDark : oneLight}
          showLineNumbers={showLineNumbersState}
          wrapLines={true}
          customStyle={{
            margin: 0,
            padding: '1rem',
            fontSize: '0.875rem',
            background: isDarkMode ? '#282c34' : '#fafafa',
          }}
          lineNumberStyle={{
            minWidth: '2.5em',
            paddingRight: '1em',
            color: isDarkMode ? '#636d83' : '#999',
            textAlign: 'right',
          }}
        >
          {code}
        </SyntaxHighlighter>
      </div>

      {/* Collapse indicator */}
      {!expanded && needsCollapse && (
        <div
          className="text-center py-2 bg-muted/30 border-t text-xs text-muted-foreground cursor-pointer hover:bg-muted/50 transition-colors"
          onClick={() => setExpanded(true)}
        >
          <span className="flex items-center justify-center gap-1">
            <span>⬇</span>
            <span>展开查看更多 ({lineCount - 10} 行)</span>
          </span>
        </div>
      )}
    </div>
  );
}

// Inline code component
export function InlineCode({ children }: { children: React.ReactNode }) {
  return (
    <code className="px-1.5 py-0.5 bg-muted rounded text-sm font-mono">
      {children}
    </code>
  );
}