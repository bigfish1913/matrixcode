/**
 * Custom hook for managing scroll behavior in chat view
 * Extracts scroll logic from ChatView to reduce complexity
 */

import { useState, useCallback, useEffect, useRef } from 'react';

export interface UseScrollManagerResult {
  autoScroll: boolean;
  setAutoScroll: (value: boolean) => void;
  scrollOffset: number;
  fineScrollUp: () => void;
  fineScrollDown: () => void;
  scrollToTop: () => void;
  scrollToBottom: () => void;
  handleScrollEvent: (event: React.UIEvent<HTMLDivElement>) => void;
}

/**
 * Hook for managing auto-scroll and fine scroll control
 * Matches TUI Alt+Up/Down behavior
 */
export function useScrollManager(
  messagesRef: React.RefObject<HTMLDivElement>
): UseScrollManagerResult {
  const [autoScroll, setAutoScroll] = useState(true);
  const [scrollOffset, setScrollOffset] = useState(0);

  // Apply scroll offset when not auto-scrolling
  useEffect(() => {
    if (!autoScroll && messagesRef.current) {
      messagesRef.current.scrollTop = scrollOffset;
    }
  }, [scrollOffset, autoScroll, messagesRef]);

  const fineScrollUp = useCallback(() => {
    if (autoScroll) {
      // Disable auto scroll first
      setAutoScroll(false);
      // Set to current max scroll position
      if (messagesRef.current) {
        const maxScroll = messagesRef.current.scrollHeight - messagesRef.current.clientHeight;
        setScrollOffset(maxScroll);
      }
    }
    // Scroll up 1 line (approximately 20px)
    setScrollOffset(prev => Math.max(0, prev - 20));
  }, [autoScroll, messagesRef]);

  const fineScrollDown = useCallback(() => {
    if (!autoScroll) {
      // Scroll down 1 line
      setScrollOffset(prev => {
        const maxScroll = messagesRef.current
          ? messagesRef.current.scrollHeight - messagesRef.current.clientHeight
          : 0;
        const newOffset = Math.min(maxScroll, prev + 20);
        // If reached bottom, enable auto scroll again
        if (newOffset >= maxScroll) {
          setAutoScroll(true);
          return 0;
        }
        return newOffset;
      });
    }
  }, [autoScroll, messagesRef]);

  const scrollToTop = useCallback(() => {
    if (messagesRef.current) {
      messagesRef.current.scrollTo({ top: 0, behavior: 'smooth' });
    }
  }, [messagesRef]);

  const scrollToBottom = useCallback(() => {
    if (messagesRef.current) {
      messagesRef.current.scrollTo({
        top: messagesRef.current.scrollHeight,
        behavior: 'smooth',
      });
    }
  }, [messagesRef]);

  const handleScrollEvent = useCallback((event: React.UIEvent<HTMLDivElement>) => {
    const target = event.currentTarget;
    const { scrollTop, scrollHeight, clientHeight } = target;
    const maxScroll = scrollHeight - clientHeight;

    // If user scrolls to bottom, re-enable auto scroll
    if (scrollTop >= maxScroll - 10) {
      setAutoScroll(true);
      setScrollOffset(0);
    } else {
      // If user scrolls away from bottom, disable auto scroll
      setAutoScroll(false);
      setScrollOffset(scrollTop);
    }
  }, []);

  return {
    autoScroll,
    setAutoScroll,
    scrollOffset,
    fineScrollUp,
    fineScrollDown,
    scrollToTop,
    scrollToBottom,
    handleScrollEvent,
  };
}