import React, { useCallback, useEffect, useRef, useState, useMemo } from 'react';
import { useChatStore } from '../stores/chatStore';
import { MessageBubble } from './MessageBubble';
import { VirtualScroll } from './VirtualScroll';
import { useToastContext } from '../contexts/ToastContext';

// Loading spinner component for thinking state
function ThinkingIndicator({ message }: { message?: string | null }) {
  return (
    <div className="flex items-center gap-2 px-4 py-3 text-muted-foreground animate-fade-in">
      <div className="flex gap-1">
        <span className="w-2 h-2 rounded-full bg-primary animate-bounce [animation-delay:-0.3s]" />
        <span className="w-2 h-2 rounded-full bg-primary animate-bounce [animation-delay:-0.15s]" />
        <span className="w-2 h-2 rounded-full bg-primary animate-bounce" />
      </div>
      <span className="text-sm">{message || '思考中...'}</span>
    </div>
  );
}

// Stop button icon (square)
function StopIcon() {
  return (
    <svg
      className="w-4 h-4"
      fill="none"
      stroke="currentColor"
      viewBox="0 0 24 24"
    >
      <rect x="6" y="6" width="12" height="12" rx="2" strokeWidth="2" />
    </svg>
  );
}

// Scroll navigation buttons with percentage indicator
function ScrollNav({ messagesRef, messageCount }: {
  messagesRef: React.RefObject<HTMLDivElement>;
  messageCount: number;
}) {
  const [scrollPercentage, setScrollPercentage] = useState(0);

  useEffect(() => {
    const container = messagesRef.current;
    if (!container) return;

    const updatePercentage = () => {
      const { scrollTop, scrollHeight, clientHeight } = container;
      const maxScroll = scrollHeight - clientHeight;
      const percentage = maxScroll > 0 ? (scrollTop / maxScroll) * 100 : 0;
      setScrollPercentage(percentage);
    };

    container.addEventListener('scroll', updatePercentage);
    updatePercentage();
    return () => container.removeEventListener('scroll', updatePercentage);
  }, [messagesRef]);

  const scrollToTop = () => {
    if (messagesRef.current) {
      messagesRef.current.scrollTo({ top: 0, behavior: 'smooth' });
    }
  };

  const scrollToBottom = () => {
    if (messagesRef.current) {
      messagesRef.current.scrollTo({
        top: messagesRef.current.scrollHeight,
        behavior: 'smooth'
      });
    }
  };

  return (
    <div className="flex gap-1 items-center">
      <span className="text-xs text-muted-foreground px-1">
        {Math.round(scrollPercentage)}%
      </span>
      <span className="text-xs text-muted-foreground">
        {messageCount} msgs
      </span>
      <button
        onClick={scrollToTop}
        className="p-1.5 hover:bg-accent rounded text-xs text-muted-foreground hover:text-foreground transition-colors"
        title="Scroll to top (Home)"
      >
        ▲
      </button>
      <button
        onClick={scrollToBottom}
        className="p-1.5 hover:bg-accent rounded text-xs text-muted-foreground hover:text-foreground transition-colors"
        title="Scroll to bottom (End)"
      >
        ▼
      </button>
    </div>
  );
}

// Message count indicator (matching TUI message count display)
function MessageCountIndicator({ count }: { count: number }) {
  if (count === 0) return null;

  return (
    <div className="text-xs text-muted-foreground px-2 py-1 bg-muted/30 rounded animate-fade-in">
      <span className="font-mono">{count}</span>
      <span className="ml-1">messages</span>
    </div>
  );
}

// Performance indicator showing render stats
function PerformanceIndicator({
  messageCount,
  visibleCount
}: {
  messageCount: number;
  visibleCount: number;
}) {
  if (messageCount < 100) return null;

  const savedPercentage = Math.round((1 - visibleCount / messageCount) * 100);

  return (
    <div className="text-xs text-green-500 px-2 py-1 bg-green-500/10 rounded animate-fade-in">
      <span>⚡</span>
      <span className="ml-1 font-mono">{savedPercentage}%</span>
      <span className="ml-1">render saved</span>
    </div>
  );
}

export function ChatView() {
  const messages = useChatStore((s) => s.messages);
  const status = useChatStore((s) => s.status);
  const sendMessage = useChatStore((s) => s.sendMessage);
  const retryLastMessage = useChatStore((s) => s.retryLastMessage);
  const stopAgent = useChatStore((s) => s.stopAgent);
  const startListening = useChatStore((s) => s.startListening);
  const inputTokens = useChatStore((s) => s.inputTokens);
  const outputTokens = useChatStore((s) => s.outputTokens);
  const cacheReadTokens = useChatStore((s) => s.cacheReadTokens);
  const cacheCreationTokens = useChatStore((s) => s.cacheCreationTokens);
  const progressMessage = useChatStore((s) => s.progressMessage);

  const toast = useToastContext();

  const [input, setInput] = useState('');
  const [useVirtualScroll, setUseVirtualScroll] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const messagesRef = useRef<HTMLDivElement>(null);
  const [visibleMessageCount, setVisibleMessageCount] = useState(0);

  const stopListening = useChatStore((s) => s.stopListening);

  // Auto-enable virtual scroll when message count exceeds threshold
  useEffect(() => {
    setUseVirtualScroll(messages.length > 50);
  }, [messages.length]);

  // Start listening for agent events on mount, cleanup on unmount
  useEffect(() => {
    startListening();
    return () => {
      stopListening();
    };
  }, [startListening, stopListening]);

  // Auto-scroll to bottom when new messages arrive or status changes
  useEffect(() => {
    if (bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [messages, status, progressMessage]);

  const handleSend = () => {
    const text = input.trim();
    if (!text || status === 'running') return;
    setInput('');
    sendMessage(text);
  };

  const formatTokenCount = (count: number): string => {
    if (count >= 1000) return `${(count / 1000).toFixed(1)}k`;
    return String(count);
  };

  // Estimate message height for virtual scroll
  const estimateMessageHeight = useCallback((index: number, message: typeof messages[0]): number => {
    // Base height
    let height = 80; // Minimum message bubble height

    // Add height for content
    const contentLength = message.content?.length || 0;
    height += Math.ceil(contentLength / 50) * 20; // ~20px per 50 chars

    // Add height for thinking block
    if (message.thinking) {
      height += 60 + Math.ceil(message.thinking.length / 30) * 15;
    }

    // Add height for tool calls
    if (message.role === 'tool') {
      height += 40;
    }

    return Math.min(height, 500); // Cap at 500px
  }, []);

  // Render single message
  const renderMessage = useCallback((message: typeof messages[0], index: number) => {
    return (
      <MessageBubble
        key={message.id}
        message={message}
        isLast={index === messages.length - 1}
        onRetry={message.isError ? retryLastMessage : undefined}
      />
    );
  }, [retryLastMessage, messages.length]);

  // Track visible messages count for performance indicator
  const handleVirtualScroll = useCallback((scrollTop: number) => {
    // Calculate visible count (approximate)
    const containerHeight = 600; // Approximate container height
    const avgHeight = 100;
    const visibleCount = Math.ceil(containerHeight / avgHeight) + 10; // +10 for overscan
    setVisibleMessageCount(visibleCount);
  }, []);

  return (
    <div className="flex flex-col h-full">
      {/* Messages area */}
      <div ref={messagesRef} className="flex-1 overflow-y-auto scrollbar-thin">
        {messages.length === 0 ? (
          <div className="flex items-center justify-center h-full text-muted-foreground">
            <div className="text-center animate-fade-in">
              <p className="text-lg mb-2">No messages yet</p>
              <p className="text-sm">Start a conversation by typing below</p>
            </div>
          </div>
        ) : (
          <>
            {/* Scroll navigation and indicators */}
            <div className="sticky top-0 right-0 float-right p-2 z-10 bg-background/80 backdrop-blur-sm rounded-bl-lg">
              <ScrollNav messagesRef={messagesRef} messageCount={messages.length} />
            </div>

            {/* Message count and performance indicators */}
            <div className="sticky top-0 left-0 float-left p-2 z-10 space-y-1">
              <MessageCountIndicator count={messages.length} />
              {useVirtualScroll && (
                <PerformanceIndicator
                  messageCount={messages.length}
                  visibleCount={visibleMessageCount}
                />
              )}
            </div>

            <div className="max-w-4xl mx-auto px-4 py-4 pt-16">
              {useVirtualScroll ? (
                /* Virtual scroll for large message lists (performance optimization) */
                <VirtualScroll
                  items={messages}
                  itemHeight={estimateMessageHeight}
                  containerHeight={600}
                  renderItem={renderMessage}
                  overscan={10}
                  getItemKey={(msg) => msg.id}
                  onScroll={handleVirtualScroll}
                />
              ) : (
                /* Standard rendering for small message lists */
                messages.map((msg, idx) => (
                  <MessageBubble
                    key={msg.id}
                    message={msg}
                    isLast={idx === messages.length - 1}
                    onRetry={msg.isError ? retryLastMessage : undefined}
                  />
                ))
              )}

              {/* Thinking indicator when agent is running */}
              {status === 'running' && <ThinkingIndicator message={progressMessage} />}
              <div ref={bottomRef} />
            </div>
          </>
        )}
      </div>

      {/* Token usage bar */}
      {(inputTokens > 0 || outputTokens > 0) && status === 'running' && (
        <div className="px-4 py-2 text-xs text-muted-foreground border-t flex items-center gap-3 animate-fade-in">
          <span>Processing... {formatTokenCount(inputTokens + outputTokens)} tokens</span>
          {cacheReadTokens > 0 && (
            <span className="text-green-500">
              ⚡ {formatTokenCount(cacheReadTokens)} cached
            </span>
          )}
        </div>
      )}

      {/* Input area */}
      <div className="border-t p-4">
        <div className="max-w-4xl mx-auto">
          <textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              // Enter: send message
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                handleSend();
              }
              // Escape: interrupt agent or clear input
              if (e.key === 'Escape' && !e.shiftKey) {
                e.preventDefault();
                if (status === 'running') {
                  // Interrupt agent
                  stopAgent();
                  toast.addToast({ type: 'warning', message: '⚡ 已中断' });
                } else if (input.trim()) {
                  // Clear input when idle
                  setInput('');
                }
              }
              // Shift+Escape: clear pending queue
              if (e.key === 'Escape' && e.shiftKey) {
                e.preventDefault();
                const pendingMessages = useChatStore.getState().pendingMessages;
                if (pendingMessages.length > 0) {
                  useChatStore.getState().clearPendingMessages();
                  toast.addToast({ type: 'info', message: `已清除 ${pendingMessages.length} 条排队消息` });
                }
              }
            }}
            placeholder={status === 'running' ? 'Agent is thinking...' : 'Type a message...'}
            disabled={status === 'running'}
            className="w-full px-3 py-2 bg-background border border-input rounded-lg text-sm resize-none focus:outline-none focus:ring-2 focus:ring-primary/50 disabled:opacity-50"
            rows={2}
          />
          <div className="flex justify-between items-center mt-2">
            <span className="text-xs text-muted-foreground">
              Press Enter to send, Shift+Enter for newline
            </span>
            <div className="flex gap-2">
              {status === 'running' ? (
                <button
                  onClick={stopAgent}
                  className="px-4 py-2 bg-destructive text-destructive-foreground rounded-lg text-sm font-medium hover:bg-destructive/90 transition-colors flex items-center gap-1.5 animate-fade-in"
                >
                  <StopIcon />
                  Stop
                </button>
              ) : (
                <button
                  onClick={handleSend}
                  disabled={!input.trim()}
                  className="px-4 py-2 bg-primary text-primary-foreground rounded-lg text-sm font-medium hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  Send
                </button>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}