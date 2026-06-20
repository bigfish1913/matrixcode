/**
 * Custom hook for managing chat input state
 * Extracts input logic from ChatView to reduce complexity
 */

import { useState, useCallback } from 'react';
import { useChatStore } from '../stores/chatStore';

export interface UseChatInputResult {
  input: string;
  setInput: (value: string) => void;
  historyIndex: number;
  navigateHistoryUp: () => void;
  navigateHistoryDown: () => void;
  resetHistory: () => void;
  addToHistory: (text: string) => void;
}

/**
 * Hook for managing chat input with history navigation
 */
export function useChatInput(): UseChatInputResult {
  const [input, setInput] = useState('');
  const [historyIndex, setHistoryIndex] = useState(-1);
  const inputHistory = useChatStore((s) => s.inputHistory);
  const addToHistoryStore = useChatStore((s) => s.addToHistory);

  const navigateHistoryUp = useCallback(() => {
    if (inputHistory.length > 0 && historyIndex < inputHistory.length - 1) {
      const newIndex = historyIndex + 1;
      setHistoryIndex(newIndex);
      setInput(inputHistory[inputHistory.length - 1 - newIndex]);
    }
  }, [inputHistory, historyIndex]);

  const navigateHistoryDown = useCallback(() => {
    if (historyIndex > 0) {
      const newIndex = historyIndex - 1;
      setHistoryIndex(newIndex);
      setInput(inputHistory[inputHistory.length - 1 - newIndex]);
    } else if (historyIndex === 0) {
      setHistoryIndex(-1);
      setInput('');
    }
  }, [inputHistory, historyIndex]);

  const resetHistory = useCallback(() => {
    setHistoryIndex(-1);
  }, []);

  const addToHistory = useCallback((text: string) => {
    if (text.trim().length > 0) {
      addToHistoryStore(text.trim());
    }
  }, [addToHistoryStore]);

  return {
    input,
    setInput,
    historyIndex,
    navigateHistoryUp,
    navigateHistoryDown,
    resetHistory,
    addToHistory,
  };
}