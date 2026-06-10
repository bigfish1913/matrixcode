import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useChatStore } from '../stores/chatStore';
import { MessageBubble } from './MessageBubble';

// Loading spinner component for thinking state
function ThinkingIndicator() {
  return (
    <div className="flex items-center gap-2 px-4 py-3 text-muted-foreground">
      <div className="flex gap-1">
        <span className="w-2 h-2 rounded-full bg-primary animate-bounce [animation-delay:-0.3s]" />
        <span className="w-2 h-2 rounded-full bg-primary animate-bounce [animation-delay:-0.15s]" />
        <span className="w-2 h-2 rounded-full bg-primary animate-bounce" />
      </div>
      <span className="text-sm">思考中...</span>
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

export function ChatView() {
  const messages = useChatStore((s) => s.messages);
  const status = useChatStore((s) => s.status);
  const sendMessage = useChatStore((s) => s.sendMessage);
  const stopAgent = useChatStore((s) => s.stopAgent);
  const startListening = useChatStore((s) => s.startListening);
  const inputTokens = useChatStore((s) => s.inputTokens);
  const outputTokens = useChatStore((s) => s.outputTokens);
  const [input, setInput] = useState('');
  const bottomRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const stopListening = useChatStore((s) => s.stopListening);

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
  }, [messages, status]);

  // Auto-resize textarea based on content
  const adjustTextareaHeight = useCallback(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      textarea.style.height = `${Math.min(textarea.scrollHeight, 200)}px`;
    }
  }, []);

  useEffect(() => {
    adjustTextareaHeight();
  }, [input, adjustTextareaHeight]);

  const handleSend = () => {
    const text = input.trim();
    if (!text || status === 'running') return;
    setInput('');
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
    sendMessage(text);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const formatTokenCount = (count: number): string => {
    if (count >= 1000) return `${(count / 1000).toFixed(1)}k`;
    return String(count);
  };

  return (
    <div className="flex flex-col h-full">
      {/* Messages area */}
      <div className="flex-1 overflow-y-auto">
        {messages.length === 0 && (
          <div className="flex items-center justify-center h-full text-muted-foreground">
            <div className="text-center space-y-3">
              <div className="text-4xl">⚡</div>
              <h2 className="text-2xl font-bold">MatrixCode</h2>
              <p className="text-sm max-w-md">
                AI-powered code assistant. Start a conversation by typing below,
                or use the sidebar to continue a previous session.
              </p>
              <div className="flex gap-2 justify-center text-xs text-muted-foreground/70 pt-2">
                <kbd className="px-1.5 py-0.5 border rounded text-xs">Enter</kbd>
                <span>to send</span>
                <kbd className="px-1.5 py-0.5 border rounded text-xs">Shift+Enter</kbd>
                <span>new line</span>
              </div>
            </div>
          </div>
        )}
        <div className="max-w-4xl mx-auto px-4 py-4">
          {messages.map((msg, idx) => (
            <MessageBubble key={msg.id} message={msg} isLast={idx === messages.length - 1} />
          ))}
          {/* Thinking indicator when agent is running */}
          {status === 'running' && <ThinkingIndicator />}
          <div ref={bottomRef} />
        </div>
      </div>

      {/* Token usage bar */}
      {(inputTokens > 0 || outputTokens > 0) && (
        <div className="px-4 py-1 text-xs text-muted-foreground border-t flex items-center gap-3">
          <span>Tokens:</span>
          <span>In: {formatTokenCount(inputTokens)}</span>
          <span>Out: {formatTokenCount(outputTokens)}</span>
          <span>Total: {formatTokenCount(inputTokens + outputTokens)}</span>
        </div>
      )}

      {/* Input area */}
      <div className="border-t p-4">
        <div className="max-w-4xl mx-auto">
          <div className="flex gap-2 items-end">
            <textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={
                status === 'running'
                  ? 'Agent is thinking...'
                  : 'Type a message...'
              }
              disabled={status === 'running'}
              rows={1}
              className="flex-1 resize-none rounded-lg border bg-background px-3 py-2.5 text-sm focus:outline-none focus:ring-2 focus:ring-ring disabled:opacity-50 min-h-[40px] max-h-[200px]"
            />
            {status === 'running' ? (
              <button
                onClick={stopAgent}
                className="px-4 py-2.5 bg-destructive text-destructive-foreground rounded-lg text-sm font-medium hover:bg-destructive/90 transition-colors shrink-0 flex items-center gap-1.5"
              >
                <StopIcon />
                Stop
              </button>
            ) : (
              <button
                onClick={handleSend}
                disabled={!input.trim()}
                className="px-4 py-2.5 bg-primary text-primary-foreground rounded-lg text-sm font-medium hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors shrink-0"
              >
                Send
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
