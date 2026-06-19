import React, { useEffect, useRef, useState } from 'react';

interface ScrollManagerProps {
  containerRef: React.RefObject<HTMLDivElement>;
  hasNewContent: boolean;
  onNewContentViewed?: () => void;
}

// Scroll management component (matching TUI scroll behavior)
export function ScrollManager({
  containerRef,
  hasNewContent,
  onNewContentViewed,
}: ScrollManagerProps) {
  const [autoScroll, setAutoScroll] = useState(true);
  const [showScrollNotification, setShowScrollNotification] = useState(false);
  const [scrollPosition, setScrollPosition] = useState(0);
  const lastScrollTop = useRef(0);

  // Check scroll position
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const handleScroll = () => {
      const { scrollTop, scrollHeight, clientHeight } = container;
      const isAtBottom = scrollHeight - scrollTop - clientHeight < 50;

      // Update scroll position
      setScrollPosition(scrollTop);

      // If scrolled up manually, disable auto-scroll
      if (scrollTop < lastScrollTop.current && !isAtBottom) {
        setAutoScroll(false);
      }

      // If scrolled to bottom, enable auto-scroll
      if (isAtBottom) {
        setAutoScroll(true);
        setShowScrollNotification(false);
        if (hasNewContent && onNewContentViewed) {
          onNewContentViewed();
        }
      }

      lastScrollTop.current = scrollTop;
    };

    container.addEventListener('scroll', handleScroll);
    return () => container.removeEventListener('scroll', handleScroll);
  }, [containerRef, hasNewContent, onNewContentViewed]);

  // Show notification when new content arrives while scrolled up
  useEffect(() => {
    if (hasNewContent && !autoScroll) {
      setShowScrollNotification(true);
    }
  }, [hasNewContent, autoScroll]);

  // Auto-scroll to bottom when new content arrives (if autoScroll is enabled)
  useEffect(() => {
    if (autoScroll && hasNewContent && containerRef.current) {
      containerRef.current.scrollTo({
        top: containerRef.current.scrollHeight,
        behavior: 'smooth',
      });
    }
  }, [autoScroll, hasNewContent, containerRef]);

  // Scroll to bottom
  const scrollToBottom = () => {
    if (containerRef.current) {
      containerRef.current.scrollTo({
        top: containerRef.current.scrollHeight,
        behavior: 'smooth',
      });
      setAutoScroll(true);
      setShowScrollNotification(false);
      if (onNewContentViewed) {
        onNewContentViewed();
      }
    }
  };

  // Scroll to top
  const scrollToTop = () => {
    if (containerRef.current) {
      containerRef.current.scrollTo({
        top: 0,
        behavior: 'smooth',
      });
      setAutoScroll(false);
    }
  };

  // Scroll by page
  const scrollByPage = (direction: 'up' | 'down') => {
    if (containerRef.current) {
      const pageHeight = containerRef.current.clientHeight * 0.9;
      containerRef.current.scrollBy({
        top: direction === 'down' ? pageHeight : -pageHeight,
        behavior: 'smooth',
      });
    }
  };

  return {
    autoScroll,
    showScrollNotification,
    scrollPosition,
    scrollToBottom,
    scrollToTop,
    scrollByPage,
  };
}

// Scroll navigation buttons component
export function ScrollNavButtons({
  containerRef,
  onScrollToTop,
  onScrollToBottom,
}: {
  containerRef: React.RefObject<HTMLDivElement>;
  onScrollToTop?: () => void;
  onScrollToBottom?: () => void;
}) {
  const [scrollPercentage, setScrollPercentage] = useState(0);

  // Calculate scroll percentage
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const updatePercentage = () => {
      const { scrollTop, scrollHeight, clientHeight } = container;
      const maxScroll = scrollHeight - clientHeight;
      const percentage = maxScroll > 0 ? (scrollTop / maxScroll) * 100 : 0;
      setScrollPercentage(percentage);
    };

    container.addEventListener('scroll', updatePercentage);
    updatePercentage();  // Initial calculation
    return () => container.removeEventListener('scroll', updatePercentage);
  }, [containerRef]);

  return (
    <div className="flex gap-1 items-center">
      {/* Scroll percentage indicator */}
      <div className="text-xs text-muted-foreground px-1">
        {Math.round(scrollPercentage)}%
      </div>

      {/* Scroll to top */}
      <button
        onClick={() => {
          if (onScrollToTop) onScrollToTop();
          else if (containerRef.current) {
            containerRef.current.scrollTo({ top: 0, behavior: 'smooth' });
          }
        }}
        className="p-1 hover:bg-accent rounded text-xs text-muted-foreground hover:text-foreground transition-colors"
        title="Scroll to top (Home)"
      >
        ▲
      </button>

      {/* Page up */}
      <button
        onClick={() => {
          if (containerRef.current) {
            containerRef.current.scrollBy({
              top: -containerRef.current.clientHeight * 0.9,
              behavior: 'smooth',
            });
          }
        }}
        className="p-1 hover:bg-accent rounded text-xs text-muted-foreground hover:text-foreground transition-colors"
        title="Page up (PageUp)"
      >
        ⇞
      </button>

      {/* Page down */}
      <button
        onClick={() => {
          if (containerRef.current) {
            containerRef.current.scrollBy({
              top: containerRef.current.clientHeight * 0.9,
              behavior: 'smooth',
            });
          }
        }}
        className="p-1 hover:bg-accent rounded text-xs text-muted-foreground hover:text-foreground transition-colors"
        title="Page down (PageDown)"
      >
        ⇟
      </button>

      {/* Scroll to bottom */}
      <button
        onClick={() => {
          if (onScrollToBottom) onScrollToBottom();
          else if (containerRef.current) {
            containerRef.current.scrollTo({
              top: containerRef.current.scrollHeight,
              behavior: 'smooth',
            });
          }
        }}
        className="p-1 hover:bg-accent rounded text-xs text-muted-foreground hover:text-foreground transition-colors"
        title="Scroll to bottom (End)"
      >
        ▼
      </button>
    </div>
  );
}

// Scroll notification component (shown when new content arrives while scrolled up)
export function ScrollNotification({
  show,
  onScrollToBottom,
}: {
  show: boolean;
  onScrollToBottom: () => void;
}) {
  if (!show) return null;

  return (
    <div className="fixed bottom-24 right-4 z-30">
      <button
        onClick={onScrollToBottom}
        className="px-3 py-1.5 bg-primary text-primary-foreground rounded-lg shadow-lg text-sm hover:bg-primary/90 transition-colors flex items-center gap-1.5 animate-pulse"
      >
        <span>▼</span>
        <span>New content</span>
      </button>
    </div>
  );
}