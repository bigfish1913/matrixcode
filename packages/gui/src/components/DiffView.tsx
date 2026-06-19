import React from 'react';

interface DiffLine {
  type: 'add' | 'remove' | 'context';
  content: string;
  oldNum?: number;
  newNum?: number;
}

interface DiffViewProps {
  diffContent: string;
  maxLines?: number;
}

// Parse unified diff format
function parseDiff(content: string): DiffLine[] {
  const lines: DiffLine[] = [];
  let oldNum = 0;
  let newNum = 0;

  // Split by lines
  const rawLines = content.split('\n');

  for (const line of rawLines) {
    if (line.startsWith('@@')) {
      // Reset line numbers for new hunk
      const match = line.match(/@@ -(\d+),?\d* \+(\d+),?\d* @@/);
      if (match) {
        oldNum = parseInt(match[1], 10);
        newNum = parseInt(match[2], 10);
      }
      lines.push({ type: 'context', content: line });
    } else if (line.startsWith('+') && !line.startsWith('+++')) {
      lines.push({
        type: 'add',
        content: line.slice(1),
        newNum: newNum++,
      });
    } else if (line.startsWith('-') && !line.startsWith('---')) {
      lines.push({
        type: 'remove',
        content: line.slice(1),
        oldNum: oldNum++,
      });
    } else if (line.startsWith(' ') || line === '') {
      lines.push({
        type: 'context',
        content: line.startsWith(' ') ? line.slice(1) : '',
        oldNum: oldNum++,
        newNum: newNum++,
      });
    } else {
      // Header lines (---, +++)
      lines.push({ type: 'context', content: line });
    }
  }

  return lines;
}

// Single diff line component
function DiffLineView({ line }: { line: DiffLine }) {
  const bgColors: Record<string, string> = {
    add: 'bg-green-500/10',
    remove: 'bg-red-500/10',
    context: '',
  };

  const textColors: Record<string, string> = {
    add: 'text-green-600 dark:text-green-400',
    remove: 'text-red-600 dark:text-red-400',
    context: 'text-muted-foreground',
  };

  const prefixes: Record<string, string> = {
    add: '+',
    remove: '-',
    context: ' ',
  };

  return (
    <div className={`flex items-center gap-2 ${bgColors[line.type]} px-2 py-0.5 font-mono text-xs`}>
      {/* Line numbers */}
      {line.oldNum !== undefined && (
        <span className="w-6 text-right text-muted-foreground/50 select-none">
          {line.oldNum}
        </span>
      )}
      {line.newNum !== undefined && (
        <span className="w-6 text-right text-muted-foreground/50 select-none border-l pl-2">
          {line.newNum}
        </span>
      )}
      {/* Prefix */}
      <span className={`${textColors[line.type]} select-none`}>
        {prefixes[line.type]}
      </span>
      {/* Content */}
      <span className={textColors[line.type]}>
        {line.content || ' '}
      </span>
    </div>
  );
}

// Hunk header (@@ line)
function HunkHeader({ content }: { content: string }) {
  return (
    <div className="px-2 py-1 bg-cyan-500/10 text-cyan-600 dark:text-cyan-400 font-mono text-xs">
      {content}
    </div>
  );
}

// Diff file header (--- a/file or +++ b/file)
function DiffFileHeader({ content }: { content: string }) {
  const isOld = content.startsWith('---');
  const color = isOld ? 'text-red-500' : 'text-green-500';

  return (
    <div className={`px-2 py-0.5 ${color} font-mono text-xs`}>
      {content}
    </div>
  );
}

export function DiffView({ diffContent, maxLines = 200 }: DiffViewProps) {
  const lines = parseDiff(diffContent);

  // Limit lines for performance
  const displayLines = lines.slice(0, maxLines);
  const hasMore = lines.length > maxLines;

  // Stats
  const added = lines.filter(l => l.type === 'add').length;
  const removed = lines.filter(l => l.type === 'remove').length;

  return (
    <div className="bg-card border rounded overflow-hidden">
      {/* Stats header */}
      <div className="px-3 py-2 bg-muted/30 border-b flex items-center gap-4 text-xs">
        <span className="flex items-center gap-1">
          <span className="text-green-500">+{added}</span>
          <span className="text-muted-foreground">添加</span>
        </span>
        <span className="flex items-center gap-1">
          <span className="text-red-500">-{removed}</span>
          <span className="text-muted-foreground">删除</span>
        </span>
        <span className="text-muted-foreground flex-1">
          {lines.filter(l => l.type === 'context' && !l.content.startsWith('@@') && !l.content.startsWith('---') && !l.content.startsWith('+++')).length} 上下文行
        </span>
        {hasMore && (
          <span className="text-yellow-500">
            显示 {maxLines}/{lines.length} 行
          </span>
        )}
      </div>

      {/* Diff content */}
      <div className="overflow-y-auto max-h-[400px]">
        {displayLines.map((line, idx) => {
          // Special handling for headers
          if (line.content.startsWith('@@')) {
            return <HunkHeader key={idx} content={line.content} />;
          }
          if (line.content.startsWith('---') || line.content.startsWith('+++')) {
            return <DiffFileHeader key={idx} content={line.content} />;
          }

          return <DiffLineView key={idx} line={line} />;
        })}
      </div>
    </div>
  );
}