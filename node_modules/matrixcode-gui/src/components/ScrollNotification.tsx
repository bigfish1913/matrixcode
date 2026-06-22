import React, { useEffect, useState } from 'react';

interface ScrollNotificationProps {
  messagesRef: React.RefObject<HTMLDivElement>;
  hasNewMessage: boolean;
  onViewNewMessage: () => void;
}

// Scroll notification matching TUI behavior
export function ScrollNotification({
  messagesRef,
  hasNewMessage,
  onViewNewMessage,
}: ScrollNotificationProps) {
  const [isScrolledUp, setIsScrolledUp] = useState(false);
  const [showNewMessageIndicator, setShowNewMessageIndicator] = useState(false);

  // Detect scroll position
  useEffect(() => {
    const container = messagesRef.current;
    if (!container) return;

    const handleScroll = () => {
      const { scrollTop, scrollHeight, clientHeight } = container;
      const isAtBottom = scrollHeight - scrollTop - clientHeight < 50; // 50px threshold
      setIsScrolledUp(!isAtBottom);

      // Clear new message indicator when scrolled to bottom
      if (isAtBottom && showNewMessageIndicator) {
        setShowNewMessageIndicator(false);
      }
    };

    container.addEventListener('scroll', handleScroll);
    return () => container.removeEventListener('scroll', handleScroll);
  }, [messagesRef, showNewMessageIndicator]);

  // Show new message indicator when hasNewMessage and scrolled up
  useEffect(() => {
    if (hasNewMessage && isScrolledUp) {
      setShowNewMessageIndicator(true);
    }
  }, [hasNewMessage, isScrolledUp]);

  // Scroll to bottom handler
  const scrollToBottom = () => {
    if (messagesRef.current) {
      messagesRef.current.scrollTo({
        top: messagesRef.current.scrollHeight,
        behavior: 'smooth',
      });
    }
    setShowNewMessageIndicator(false);
    onViewNewMessage();
  };

  // Don't render if at bottom and no new messages
  if (!isScrolledUp && !showNewMessageIndicator) return null;

  return (
    <div className="fixed bottom-[120px] right-4 z-20">
      {showNewMessageIndicator ? (
        <button
          onClick={scrollToBottom}
          className="px-3 py-2 bg-primary text-primary-foreground rounded-full shadow-lg text-xs font-medium flex items-center gap-2 animate-bounce hover:bg-primary/90 transition-colors"
        >
          <span>📥</span>
          <span>新消息!</span>
          <kbd className="px-1.5 py-0.5 bg-primary-foreground/20 rounded text-xs">End</kbd>
        </button>
      ) : (
        <button
          onClick={scrollToBottom}
          className="px-3 py-1.5 bg-muted text-muted-foreground rounded-full shadow text-xs flex items-center gap-2 hover:bg-accent transition-colors"
        >
          <span>↑</span>
          <span>滚动中</span>
          <kbd className="px-1.5 py-0.5 bg-background rounded text-xs">End</kbd>
          <span className="text-muted-foreground/50">底部</span>
        </button>
      )}
    </div>
  );
}