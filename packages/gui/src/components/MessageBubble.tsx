import React, { type ReactNode, useState, useEffect } from 'react';
import ReactMarkdown from 'react-markdown';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism';
import type { ChatMessage } from '../stores/chatStore';
import { getToolIcon, formatToolName } from '../utils/toolIcons';

interface Props {
  message: ChatMessage;
  isLast?: boolean;
  onRetry?: () => void;
}

// Format timestamp to readable time
function formatTime(timestamp?: number): string {
  if (!timestamp) return '';
  const date = new Date(timestamp);
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
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
      title="Copy"
    >
      {copied ? '✓' : 'Copy'}
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
      title="Copy message"
    >
      {copied ? '✓ Copied' : 'Copy'}
    </button>
  );
}

export function MessageBubble({ message, isLast = false, onRetry }: Props) {
  const isUser = message.role === 'user';
  const isError = message.isError || message.role === 'error';
  const isTool = message.role === 'tool';
  const hasThinking = message.thinking && message.thinking.length > 0;

  // Control details open state - default: thinking/tool_call expanded, tool_result collapsed
  const [thinkingOpen, setThinkingOpen] = useState(() => {
    // Default: open if streaming or new message (no timestamp)
    return message.isThinkingStreaming || (hasThinking && !message.timestamp);
  });
  const [toolCallOpen, setToolCallOpen] = useState(true);  // Tool call default open
  const [toolResultOpen, setToolResultOpen] = useState(false);  // Tool result default closed (matching TUI)

  // Reset to default state when message changes (for new messages)
  useEffect(() => {
    if (message.isStreaming || message.isThinkingStreaming) {
      setThinkingOpen(true);  // Auto-expand when streaming
    }
  }, [message.isStreaming, message.isThinkingStreaming, message.id]);

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
        <span className="text-purple-600 dark:text-purple-400">💭 Thinking</span>
        {message.isThinkingStreaming && (
          <span className="w-1.5 h-1.5 rounded-full bg-purple-500 animate-pulse ml-1" />
        )}
        {!message.isThinkingStreaming && message.thinking && (
          <span className="text-xs text-muted-foreground ml-auto">
            {message.thinking.split('\n').length} lines
          </span>
        )}
      </summary>
      <pre className="mt-2 text-xs bg-purple-50 dark:bg-purple-900/20 p-2 rounded overflow-auto max-h-60 text-purple-900 dark:text-purple-100 animate-fade-in relative">
        <CopyButton text={message.thinking!} />
        {message.thinking}
      </pre>
    </details>
  );

  if (isTool && !message.isToolResult && message.toolName) {
    // Tool use call with icon
    const toolIcon = getToolIcon(message.toolName);
    const toolDisplayName = formatToolName(message.toolName);

    contentNode = (
      <details
        className="cursor-pointer group animate-slide-in-up"
        open={toolCallOpen || undefined}
        onToggle={(e) => setToolCallOpen(e.currentTarget.open)}
      >
        <summary className="text-xs font-mono text-muted-foreground flex items-center gap-1.5 select-none">
          <span className="text-amber-500 group-open:rotate-90 transition-transform duration-200">▶</span>
          <span className="text-lg">{toolIcon}</span>
          <span className="text-amber-600 dark:text-amber-400">{toolDisplayName}</span>
          {message.toolInput && typeof message.toolInput === 'object' && Object.keys(message.toolInput).length > 0 ? (
            <span className="text-xs text-muted-foreground ml-auto">
              {Object.keys(message.toolInput).length} params
            </span>
          ) : null}
        </summary>
        <pre className="mt-2 text-xs bg-muted p-2 rounded overflow-auto max-h-60 relative animate-fade-in">
          <CopyButton text={message.toolInput ? JSON.stringify(message.toolInput, null, 2) : ''} />
          {message.toolInput
            ? JSON.stringify(message.toolInput, null, 2)
            : ''}
        </pre>
      </details>
    );
  } else if (isTool && message.isToolResult) {
    // Tool result with icon and default collapsed state (matching TUI)
    const toolIcon = getToolIcon(message.toolName || '');
    const toolDisplayName = formatToolName(message.toolName || 'Tool');

    contentNode = (
      <details
        className="cursor-pointer group animate-slide-in-up"
        open={toolResultOpen || undefined}
        onToggle={(e) => setToolResultOpen(e.currentTarget.open)}
      >
        <summary className={`text-xs font-mono flex items-center gap-1.5 select-none ${
          message.isError
            ? 'text-red-500'
            : 'text-muted-foreground'
        }`}>
          <span className={`${message.isError ? 'text-red-500' : 'text-green-500'} group-open:rotate-90 transition-transform duration-200`}>▶</span>
          <span className="text-lg">{toolIcon}</span>
          <span className={message.isError ? 'text-red-600 dark:text-red-400' : 'text-green-600 dark:text-green-400'}>
            {toolDisplayName} Result
          </span>
          {message.isError && (
            <span className="text-xs px-1.5 py-0.5 bg-red-500/10 rounded">Error</span>
          )}
          <span className="text-xs text-muted-foreground ml-auto">
            {message.content.length} chars
          </span>
        </summary>
        <pre className={`mt-2 text-xs p-2 rounded overflow-auto max-h-60 relative animate-fade-in ${
          message.isError
            ? 'bg-red-500/5 text-red-900 dark:text-red-100 border border-red-500/20'
            : 'bg-muted'
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
      <div className="prose prose-sm dark:prose-invert max-w-none">
        <ReactMarkdown
          components={{
            code({ className, children, ...props }) {
              const match = /language-(\w+)/.exec(className || '');
              const codeString = String(children).replace(/\n$/, '');

              if (match) {
                return (
                  <div className="relative group/code">
                    <div className="flex items-center justify-between text-xs text-muted-foreground bg-muted/50 px-3 py-1 rounded-t-lg border-b">
                      <span>{match[1]}</span>
                    </div>
                    <SyntaxHighlighter
                      style={oneDark}
                      language={match[1]}
                      PreTag="div"
                      customStyle={{
                        margin: 0,
                        borderRadius: '0 0 0.5rem 0.5rem',
                        fontSize: '0.8rem',
                      }}
                    >
                      {codeString}
                    </SyntaxHighlighter>
                    <CopyButton text={codeString} />
                  </div>
                );
              }

              return (
                <code className={className} {...props}>
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
        className={`group max-w-[85%] rounded-lg px-4 py-2.5 text-sm ${
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
        {contentNode}
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
                Retry
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
