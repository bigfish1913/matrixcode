import React, { type ReactNode, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism';
import type { ChatMessage } from '../stores/chatStore';

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

  let contentNode: ReactNode;

  // Thinking block (shown before main content)
  const thinkingNode = hasThinking && (
    <details className="cursor-pointer group mb-2">
      <summary className="text-xs font-mono text-muted-foreground flex items-center gap-1.5 select-none">
        <span className="text-purple-500 group-open:rotate-90 transition-transform">▶</span>
        <span className="text-purple-600 dark:text-purple-400">💭 Thinking</span>
        {message.isThinkingStreaming && (
          <span className="w-1.5 h-1.5 rounded-full bg-purple-500 animate-pulse ml-1" />
        )}
      </summary>
      <pre className="mt-2 text-xs bg-purple-50 dark:bg-purple-900/20 p-2 rounded overflow-auto max-h-60 text-purple-900 dark:text-purple-100">
        {message.thinking}
      </pre>
    </details>
  );

  if (isTool && !message.isToolResult && message.toolName) {
    // Tool use call
    contentNode = (
      <details className="cursor-pointer group">
        <summary className="text-xs font-mono text-muted-foreground flex items-center gap-1.5 select-none">
          <span className="text-amber-500 group-open:rotate-90 transition-transform">▶</span>
          <span className="text-amber-600 dark:text-amber-400">⚡ {message.toolName}</span>
        </summary>
        <pre className="mt-2 text-xs bg-muted p-2 rounded overflow-auto max-h-60 relative">
          <CopyButton text={message.toolInput ? JSON.stringify(message.toolInput, null, 2) : ''} />
          {message.toolInput
            ? JSON.stringify(message.toolInput, null, 2)
            : ''}
        </pre>
      </details>
    );
  } else if (isTool && message.isToolResult) {
    // Tool result
    contentNode = (
      <details className="cursor-pointer group">
        <summary className="text-xs font-mono text-muted-foreground flex items-center gap-1.5 select-none">
          <span className="text-green-500 group-open:rotate-90 transition-transform">▶</span>
          <span className="text-green-600 dark:text-green-400">✓ {message.toolName} result</span>
        </summary>
        <pre className="mt-2 text-xs bg-muted p-2 rounded overflow-auto max-h-80 relative">
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
