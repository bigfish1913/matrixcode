import React, { type ReactNode, useState, useEffect, useMemo, memo } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism';
import type { ChatMessage } from '../stores/chatStore';
import { getToolIcon, formatToolName } from '../utils/toolIcons';
import { formatExecutionTime, formatTime } from '../utils/formatters';

// Pre-compiled regex for language detection
const LANGUAGE_REGEX = /language-(\w+)/;

interface Props {
  message: ChatMessage;
  isLast?: boolean;
  onRetry?: () => void;
  thinkingCollapsed?: boolean;  // Global thinking collapse state (matching TUI Alt+T)
}

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
      className="absolute top-1 right-1 px-1.5 py-0.5 text-xs text-muted-foreground hover:text-foreground bg-muted/80 rounded transition-colors"
      title="复制代码"
      aria-label={copied ? '已复制' : '复制代码到剪贴板'}
    >
      {copied ? '✓' : '复制'}
    </button>
  );
}

// Copy button for message content (not in pre block)
function MessageCopyButton({ content }: { content: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <button
      onClick={handleCopy}
      className="px-1.5 py-0.5 text-xs text-muted-foreground hover:text-foreground bg-muted/50 rounded transition-colors opacity-0 group-hover:opacity-100"
      title="复制消息"
      aria-label={copied ? '已复制' : '复制消息内容到剪贴板'}
    >
      {copied ? '✓ 已复制' : '复制'}
    </button>
  );
}

export const MessageBubble = memo(function MessageBubble({ message, isLast = false, onRetry, thinkingCollapsed }: Props) {
  const isUser = message.role === 'user';
  const isError = message.isError || message.role === 'error';
  const isTool = message.role === 'tool';
  const hasThinking = message.thinking && message.thinking.length > 0;

  // Memoize thinking line count calculation
  const thinkingLineCount = useMemo(() =>
    hasThinking ? message.thinking!.split('\n').length : 0,
    [hasThinking, message.thinking]
  );

  // Memoize tool icon and display name for tool calls
  const toolCallInfo = useMemo(() => ({
    icon: getToolIcon(message.toolName || ''),
    displayName: formatToolName(message.toolName || 'Tool'),
  }), [message.toolName]);

  // Memoize tool icon and display name for tool results
  const toolResultInfo = useMemo(() => ({
    icon: getToolIcon(message.toolName || ''),
    displayName: formatToolName(message.toolName || 'Tool'),
  }), [message.toolName]);

  // Memoize tool input JSON for display (handle both object and string types)
  const toolInputJson = useMemo(() => {
    if (!message.toolInput) return '';
    if (typeof message.toolInput === 'string') {
      // Already a string (possibly malformed JSON from streaming)
      return message.toolInput;
    }
    // Object - format as JSON
    return JSON.stringify(message.toolInput, null, 2);
  }, [message.toolInput]);

  // Control details open state with clearer logic
  const [thinkingOpen, setThinkingOpen] = useState(() => {
    // If global collapse state is set, follow it (highest priority)
    if (thinkingCollapsed !== undefined) {
      return !thinkingCollapsed;
    }
    // Default: open only when streaming or very short thinking (< 100 chars)
    return message.isThinkingStreaming || (hasThinking && message.thinking!.length < 100);
  });
  const [toolCallOpen, setToolCallOpen] = useState(true);  // Tool call default open
  const [toolResultOpen, setToolResultOpen] = useState(false);  // Tool result default closed (matching TUI)

  // Reset to default state when message changes (for new messages)
  useEffect(() => {
    if (message.isStreaming || message.isThinkingStreaming) {
      setThinkingOpen(true);  // Auto-expand when streaming
    }
  }, [message.isStreaming, message.isThinkingStreaming, message.id]);

  // Sync with global collapse state when it changes (but not when streaming)
  useEffect(() => {
    if (thinkingCollapsed !== undefined && !message.isThinkingStreaming) {
      setThinkingOpen(!thinkingCollapsed);
    }
  }, [thinkingCollapsed, message.isThinkingStreaming]);

  let contentNode: ReactNode;

  // Thinking block (shown before main content) - with improved collapse state
  const thinkingNode = hasThinking && (
    <details
      className="cursor-pointer group mb-2 animate-slide-in-up"
      open={thinkingOpen || undefined}
      onToggle={(e) => setThinkingOpen(e.currentTarget.open)}
    >
      <summary className="text-xs font-mono text-muted-foreground flex items-center gap-1.5 select-none">
        <span className="text-purple-500 group-open:rotate-90 transition-transform duration-200">▶</span>
        <span className="text-purple-600 dark:text-purple-400">💭 思考</span>
        {message.isThinkingStreaming && (
          <span className="w-1.5 h-1.5 rounded-full bg-purple-500 animate-pulse ml-1" />
        )}
        {!message.isThinkingStreaming && (
          <span className="text-xs text-muted-foreground ml-auto">
            {thinkingLineCount} 行
          </span>
        )}
      </summary>
      <pre className="mt-2 text-xs bg-purple-50 dark:bg-purple-900/20 p-2 rounded overflow-x-hidden overflow-y-auto max-h-60 text-purple-900 dark:text-purple-100 animate-fade-in relative whitespace-pre-wrap break-words">
        <CopyButton text={message.thinking!} />
        {message.thinking}
      </pre>
    </details>
  );

  if (isTool && !message.isToolResult && message.toolName) {
    // Tool use call with icon (use memoized values)
    contentNode = (
      <details
        className="cursor-pointer group animate-slide-in-up"
        open={toolCallOpen || undefined}
        onToggle={(e) => setToolCallOpen(e.currentTarget.open)}
      >
        <summary className="text-xs font-mono text-muted-foreground flex items-center gap-1.5 select-none bg-amber-50 dark:bg-amber-900/20 px-4 py-2 border-b border-amber-200 dark:border-amber-700/50 rounded-t-lg -mt-2.5">
          <span className="text-amber-500 group-open:rotate-90 transition-transform duration-200">▶</span>
          <span className="text-lg">{toolCallInfo.icon}</span>
          <span className="text-amber-600 dark:text-amber-400 font-medium">{toolCallInfo.displayName}</span>
          {(() => {
            if (!message.toolInput) return null;
            if (typeof message.toolInput === 'object' && Object.keys(message.toolInput).length > 0) {
              return <span className="text-xs text-muted-foreground ml-auto bg-amber-100 dark:bg-amber-800 px-1.5 py-0.5 rounded">{Object.keys(message.toolInput).length} 个参数</span>;
            }
            if (typeof message.toolInput === 'string' && message.toolInput.trim()) {
              return <span className="text-xs text-muted-foreground ml-auto bg-amber-100 dark:bg-amber-800 px-1.5 py-0.5 rounded">{message.toolInput.split('\n').filter(l => l.trim()).length} 行输入</span>;
            }
            return null;
          })()}
        </summary>
        <pre className="mt-0 text-xs bg-muted/50 p-2 rounded-b-lg overflow-x-hidden overflow-y-auto max-h-60 relative animate-fade-in whitespace-pre-wrap break-words border border-amber-200 dark:border-amber-700/30 border-t-0">
          <CopyButton text={toolInputJson} />
          {toolInputJson}
        </pre>
      </details>
    );
  } else if (isTool && message.isToolResult) {
    // Tool result with icon and default collapsed state (use memoized values)
    contentNode = (
      <details
        className="cursor-pointer group animate-slide-in-up"
        open={toolResultOpen || undefined}
        onToggle={(e) => setToolResultOpen(e.currentTarget.open)}
      >
        <summary className={`text-xs font-mono flex items-center gap-1.5 select-none px-4 py-2 border-b rounded-t-lg -mt-2.5 ${
          message.isError
            ? 'bg-red-50 dark:bg-red-900/20 text-red-500 border-red-200 dark:border-red-700/50'
            : 'bg-green-50 dark:bg-green-900/20 text-muted-foreground border-green-200 dark:border-green-700/50'
        }`}>
          <span className={`${message.isError ? 'text-red-500' : 'text-green-500'} group-open:rotate-90 transition-transform duration-200`}>▶</span>
          <span className="text-lg">{toolResultInfo.icon}</span>
          <span className={`font-medium ${message.isError ? 'text-red-600 dark:text-red-400' : 'text-green-600 dark:text-green-400'}`}>
            {toolResultInfo.displayName} 结果
          </span>
          {message.isError && (
            <span className="text-xs px-1.5 py-0.5 bg-red-500/10 rounded border border-red-500/30">错误</span>
          )}
          <span className="text-xs text-muted-foreground ml-auto bg-muted px-1.5 py-0.5 rounded">
            {message.content.length} 字符
          </span>
          {message.executionTime && (
            <span className="text-xs text-muted-foreground ml-1">
              {formatExecutionTime(message.executionTime)}
            </span>
          )}
        </summary>
        <pre className={`mt-0 text-xs p-2 rounded-b-lg overflow-x-hidden overflow-y-auto max-h-60 relative animate-fade-in whitespace-pre-wrap break-words ${
          message.isError
            ? 'bg-red-500/5 text-red-900 dark:text-red-100 border border-red-500/20 border-t-0'
            : 'bg-muted/50 border border-green-200 dark:border-green-700/30 border-t-0'
        }`}>
          <CopyButton text={message.content} />
          {message.content}
        </pre>
      </details>
    );
  } else if (isUser) {
    contentNode = <p className="whitespace-pre-wrap">{message.content}</p>;
  } else {
    // Assistant message with syntax-highlighted code blocks
    contentNode = (
      <div className="prose prose-sm dark:prose-invert max-w-none min-w-0 overflow-x-hidden">
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          components={{
            // Handle paragraphs
            p({ children }) {
              return <p className="mb-2 last:mb-0">{children}</p>;
            },
            // Handle links
            a({ href, children }) {
              return (
                <a
                  href={href}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-primary underline hover:text-primary/80"
                >
                  {children}
                </a>
              );
            },
            // Handle lists
            ul({ children }) {
              return <ul className="list-disc list-inside mb-2 space-y-1">{children}</ul>;
            },
            ol({ children }) {
              return <ol className="list-decimal list-inside mb-2 space-y-1">{children}</ol>;
            },
            li({ children }) {
              return <li className="ml-2">{children}</li>;
            },
            // Handle headings
            h1({ children }) {
              return <h1 className="text-lg font-bold mb-2">{children}</h1>;
            },
            h2({ children }) {
              return <h2 className="text-base font-bold mb-2">{children}</h2>;
            },
            h3({ children }) {
              return <h3 className="text-sm font-bold mb-1">{children}</h3>;
            },
            // Handle blockquotes
            blockquote({ children }) {
              return <blockquote className="border-l-4 border-muted-foreground/30 pl-3 italic text-muted-foreground">{children}</blockquote>;
            },
            // Handle code blocks and inline code
            code({ className, children, ...props }) {
              const match = /language-(\w+)/.exec(className || '');
              const codeString = String(children).replace(/\n$/, '');

              if (match) {
                // Code block with language
                return (
                  <div className="relative group/code overflow-x-auto my-2">
                    <div className="flex items-center justify-between text-xs text-muted-foreground bg-muted/50 px-3 py-1 rounded-t-lg border-b">
                      <span>{match[1]}</span>
                      <CopyButton text={codeString} />
                    </div>
                    <SyntaxHighlighter
                      style={oneDark}
                      language={match[1]}
                      PreTag="div"
                      customStyle={{
                        margin: 0,
                        borderRadius: '0 0 0.5rem 0.5rem',
                        fontSize: '0.8rem',
                        maxWidth: '100%',
                        overflowX: 'auto',
                      }}
                    >
                      {codeString}
                    </SyntaxHighlighter>
                  </div>
                );
              }

              // Inline code - check if this is truly inline (inside a paragraph)
              return (
                <code
                  className="bg-muted px-1.5 py-0.5 rounded text-sm font-mono"
                  {...props}
                >
                  {children}
                </code>
              );
            },
          }}
        >
          {message.content}
        </ReactMarkdown>
      </div>
    );
  }

  return (
    <div
      className={`flex ${isUser ? 'justify-end' : 'justify-start'} mb-4`}
    >
      <div
        className={`group max-w-[85%] min-w-0 rounded-lg px-4 py-2.5 text-sm ${
          isError
            ? 'bg-red-100 dark:bg-red-900/30 text-red-800 dark:text-red-200 border border-red-300 dark:border-red-700'
            : isUser
              ? 'bg-primary text-primary-foreground'
              : isTool
                ? 'bg-muted border'
                : 'bg-card border shadow-sm'
        }`}
      >
        {isError && (
          <div className="text-xs font-semibold mb-1 flex items-center gap-1">
            <span>⚠️</span> Error
          </div>
        )}
        {/* Thinking block for assistant messages */}
        {!isUser && !isTool && !isError && thinkingNode}
        <div className="min-w-0 overflow-x-hidden">
          {contentNode}
        </div>
        {message.isStreaming && isLast && (
          <span className="inline-block w-2 h-4 ml-1 bg-foreground/50 animate-pulse" />
        )}
        {/* Timestamp, copy button, and retry button - only for non-user messages */}
        {!isUser && (
          <div className="flex items-center justify-between mt-1 pt-1 border-t border-transparent group">
            {message.timestamp && (
              <span className="text-xs opacity-50">{formatTime(message.timestamp)}</span>
            )}
            {!isError && !isTool && !message.isStreaming && (
              <MessageCopyButton content={message.content} />
            )}
            {isError && onRetry && (
              <button
                onClick={onRetry}
                className="text-xs px-2 py-0.5 bg-red-200 dark:bg-red-800 rounded hover:bg-red-300 dark:hover:bg-red-700 transition-colors"
              >
                重试
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
});
