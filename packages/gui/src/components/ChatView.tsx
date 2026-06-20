import React, { useCallback, useEffect, useRef, useState, useMemo } from 'react';
import { useChatStore } from '../stores/chatStore';
import { MessageBubble } from './MessageBubble';
import { VirtualScroll } from './VirtualScroll';
import { useToastContext } from '../contexts/ToastContext';
import { StatusBar } from './StatusBar';
import { TodoList } from './TodoList';
import { AskQuestionDialog } from './AskQuestionDialog';
import { WorkflowPanel } from './WorkflowPanel';
import { QuickActionPanel } from './QuickActionPanel';
import { ModelSwitcherDialog } from './ModelSwitcherDialog';
// Note: LSP/MCP/CodeGraph panels are imported but managed centrally in App.tsx
import { MessageContextMenu } from './MessageContextMenu';
import { formatTokenCount } from '../utils/formatters';
import { InputShortcuts } from './shared';
import { useChatInput, useScrollManager } from '../hooks';

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
  if (messageCount < 50) return null; // Lower threshold for better UX

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
  const inputHistory = useChatStore((s) => s.inputHistory);
  const todos = useChatStore((s) => s.todos);
  const askQuestion = useChatStore((s) => s.askQuestion);
  const answerQuestion = useChatStore((s) => s.answerQuestion);
  const dismissQuestion = useChatStore((s) => s.dismissQuestion);
  const workflowState = useChatStore((s) => s.workflowState);
  const toggleWorkflowPanel = useChatStore((s) => s.toggleWorkflowPanel);

  const toast = useToastContext();

  // Panel visibility states (for StatusBar interactions - managed in App.tsx for actual panels)
  const [showModelSwitcher, setShowModelSwitcher] = useState(false);

  // Context menu state (matching VSCode extension editor/context menu)
  const [contextMenu, setContextMenu] = useState<{
    visible: boolean;
    selectedText: string;
    position: { x: number; y: number };
  }>({
    visible: false,
    selectedText: '',
    position: { x: 0, y: 0 },
  });

  // Refs must be defined before using them in hooks
  const bottomRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const messagesRef = useRef<HTMLDivElement>(null);

  // Use custom hooks for input and scroll management
  const {
    input,
    setInput,
    historyIndex,
    navigateHistoryUp,
    navigateHistoryDown,
    resetHistory,
    addToHistory,
  } = useChatInput();

  const {
    autoScroll,
    scrollOffset,
    fineScrollUp,
    fineScrollDown,
    scrollToBottom,
    handleScrollEvent,
  } = useScrollManager(messagesRef);

  const [useVirtualScroll, setUseVirtualScroll] = useState(false);
  const [showQuickActions, setShowQuickActions] = useState(true);
  const [thinkingCollapsed, setThinkingCollapsed] = useState(false);
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
    if (autoScroll && bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [messages, status, progressMessage, autoScroll]);

  const handleSend = () => {
    const text = input.trim();
    if (!text || status === 'running') return;
    setInput('');
    addToHistory(text);
    resetHistory();
    sendMessage(text);
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
        thinkingCollapsed={thinkingCollapsed}
      />
    );
  }, [retryLastMessage, messages.length, thinkingCollapsed]);

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
      {/* Status bar at top - callbacks trigger panels managed in App.tsx */}
      <StatusBar
        onOpenModelSwitcher={() => setShowModelSwitcher(true)}
        onOpenSettings={() => {
          // Open settings view (matching VSCode matrixcode.openSettings)
          // This will switch to settings tab in Sidebar
          // For now, show model switcher as fallback
          setShowModelSwitcher(true);
        }}
        // Note: MCP/LSP/CodeGraph panels are managed by App.tsx via global shortcuts
        // These callbacks are optional and will be handled by parent if provided
      />

      {/* Messages area */}
      <div ref={messagesRef} className="flex-1 overflow-y-auto scrollbar-thin"
        onContextMenu={(e) => {
          // Handle right-click on messages area (matching VSCode extension editor/context)
          e.preventDefault();

          // Get selected text from window selection
          const selection = window.getSelection();
          const selectedText = selection?.toString() || '';

          if (selectedText.trim()) {
            setContextMenu({
              visible: true,
              selectedText: selectedText.trim(),
              position: { x: e.clientX, y: e.clientY },
            });
          } else {
            // No text selected - show basic menu or close
            setContextMenu({
              visible: false,
              selectedText: '',
              position: { x: 0, y: 0 },
            });
          }
        }}
        onScroll={handleScrollEvent}
      >
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
              {/* Todo list (if active during agent run) */}
              {todos.length > 0 && status === 'running' && (
                <TodoList todos={todos} maxVisible={5} />
              )}

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

      {/* Quick action toolbar (aligns with VSCode extension commands) */}
      {showQuickActions && (
        <QuickActionPanel
          onSendMessage={sendMessage}
          collapsed={false}
          onToggleCollapse={() => setShowQuickActions(false)}
        />
      )}
      {!showQuickActions && (
        <QuickActionPanel
          onSendMessage={sendMessage}
          collapsed={true}
          onToggleCollapse={() => setShowQuickActions(true)}
        />
      )}

      {/* Input area */}
      <div className="border-t p-4">
        <div className="max-w-4xl mx-auto">
          <textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              // Arrow Up: navigate input history backwards
              if (e.key === 'ArrowUp' && !e.shiftKey && textareaRef.current === document.activeElement) {
                e.preventDefault();
                navigateHistoryUp();
              }
              // Arrow Down: navigate input history forwards
              else if (e.key === 'ArrowDown' && !e.shiftKey && textareaRef.current === document.activeElement) {
                e.preventDefault();
                navigateHistoryDown();
              }
              // Enter: send message
              else if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                resetHistory();
                handleSend();
              }
              // Alt+T: Toggle thinking collapse (matching TUI)
              else if (e.key === 't' && e.altKey) {
                e.preventDefault();
                setThinkingCollapsed(prev => !prev);
              }
              // Alt+Up/Down: Fine scroll control (matching TUI)
              else if (e.key === 'ArrowUp' && e.altKey) {
                e.preventDefault();
                fineScrollUp();
              }
              else if (e.key === 'ArrowDown' && e.altKey) {
                e.preventDefault();
                fineScrollDown();
              }
              // Ctrl+C: Copy selected text or interrupt agent (matching TUI)
              else if (e.key === 'c' && e.ctrlKey && !e.altKey && !e.shiftKey) {
                e.preventDefault();
                const selection = window.getSelection();
                const selectedText = selection?.toString() || '';
                if (selectedText.trim()) {
                  // Copy selected text to clipboard
                  try {
                    navigator.clipboard.writeText(selectedText.trim());
                    toast.addToast({ type: 'success', message: `📋 已复制 ${selectedText.length} 字符到剪贴板` });
                  } catch (err) {
                    toast.addToast({ type: 'error', message: '复制失败' });
                  }
                } else if (status === 'running') {
                  // No text selected and agent is running - interrupt
                  stopAgent();
                  toast.addToast({ type: 'warning', message: '⚡ 已中断' });
                }
              }
              // Escape: interrupt agent or clear input
              else if (e.key === 'Escape' && !e.shiftKey) {
                e.preventDefault();
                if (status === 'running') {
                  // Interrupt agent
                  stopAgent();
                  toast.addToast({ type: 'warning', message: '⚡ 已中断' });
                } else if (input.trim()) {
                  // Clear input when idle
                  setInput('');
                  resetHistory();
                }
              }
              // Shift+Escape: clear pending queue
              else if (e.key === 'Escape' && e.shiftKey) {
                e.preventDefault();
                const pendingMessages = useChatStore.getState().pendingMessages;
                if (pendingMessages.length > 0) {
                  useChatStore.getState().clearPendingMessages();
                  toast.addToast({ type: 'info', message: `已清除 ${pendingMessages.length} 条排队消息` });
                }
              }
            }}
            placeholder={status === 'running' ? 'Agent is thinking... (Esc to stop)' : 'Type a message... (↑↓ for history)'}
            disabled={status === 'running'}
            aria-label="Chat message input"
            aria-placeholder={status === 'running' ? 'Agent is processing, press Escape to stop' : 'Type your message here'}
            aria-required="true"
            aria-busy={status === 'running'}
            className="w-full px-3 py-2 bg-background border border-input rounded-lg text-sm resize-none focus:outline-none focus:ring-2 focus:ring-primary/50 disabled:opacity-50"
            rows={2}
          />
          <div className="flex justify-between items-center mt-2">
            <InputShortcuts />
            <div className="flex gap-2">
              {status === 'running' ? (
                <button
                  onClick={stopAgent}
                  aria-label="Stop agent execution"
                  aria-busy="true"
                  className="px-4 py-2 bg-destructive text-destructive-foreground rounded-lg text-sm font-medium hover:bg-destructive/90 transition-colors flex items-center gap-1.5 animate-fade-in"
                >
                  <StopIcon />
                  Stop
                </button>
              ) : (
                <button
                  onClick={handleSend}
                  disabled={!input.trim()}
                  aria-label="Send message"
                  aria-disabled={!input.trim()}
                  className="px-4 py-2 bg-primary text-primary-foreground rounded-lg text-sm font-medium hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  Send
                </button>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Ask question dialog (when agent needs user input) */}
      {askQuestion?.isVisible && (
        <AskQuestionDialog
          question={askQuestion.question}
          options={askQuestion.options}
          onAnswer={answerQuestion}
          onCancel={dismissQuestion}
        />
      )}

      {/* Workflow panel (when workflow is active) */}
      {workflowState.visible && (
        <WorkflowPanel
          workflowState={workflowState}
          onToggle={toggleWorkflowPanel}
        />
      )}

      {/* Dialogs from StatusBar interactions - Note: These are also managed in App.tsx */}
      {/* Status panels are managed centrally in App.tsx to avoid duplication */}

      {/* Context menu (matching VSCode extension editor/context) */}
      {contextMenu.visible && (
        <MessageContextMenu
          selectedText={contextMenu.selectedText}
          position={contextMenu.position}
          onClose={() => setContextMenu({ visible: false, selectedText: '', position: { x: 0, y: 0 } })}
        />
      )}
    </div>
  );
}