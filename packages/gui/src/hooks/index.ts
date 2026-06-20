/**
 * Custom hooks index
 * Centralized exports for all custom hooks
 */

export { useKeyboardShortcuts, useConditionalKeyboardShortcuts, createShortcut } from './useKeyboardShortcuts';
export type { KeyboardShortcut } from './useKeyboardShortcuts';

export { useChatInput } from './useChatInput';
export type { UseChatInputResult } from './useChatInput';

export { useScrollManager } from './useScrollManager';
export type { UseScrollManagerResult } from './useScrollManager';