/**
 * Custom hook for keyboard shortcut management
 * Reduces App.tsx complexity and centralizes keyboard handling
 */

import { useEffect } from 'react';

export interface KeyboardShortcut {
  key: string;
  modifiers?: {
    ctrl?: boolean;
    alt?: boolean;
    shift?: boolean;
    meta?: boolean;
  };
  action: () => void;
  description?: string;
  enabled?: boolean;
}

/**
 * Hook for managing global keyboard shortcuts
 * @param shortcuts - Array of keyboard shortcuts to register
 * @param enabled - Whether shortcuts are enabled (default: true)
 */
export function useKeyboardShortcuts(
  shortcuts: KeyboardShortcut[],
  enabled: boolean = true
) {
  useEffect(() => {
    if (!enabled) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Check each shortcut
      for (const shortcut of shortcuts) {
        // Check if shortcut is enabled (default: true)
        if (shortcut.enabled === false) continue;

        // Check key match
        if (e.key !== shortcut.key) continue;

        // Check modifiers
        const mods = shortcut.modifiers || {};
        const ctrlMatch = mods.ctrl ? (e.ctrlKey || e.metaKey) : !e.ctrlKey && !e.metaKey;
        const altMatch = mods.alt ? e.altKey : !e.altKey;
        const shiftMatch = mods.shift ? e.shiftKey : !e.shiftKey;

        // If all modifiers match, execute action
        if (ctrlMatch && altMatch && shiftMatch) {
          e.preventDefault();
          shortcut.action();
          break; // Only execute first matching shortcut
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [shortcuts, enabled]);
}

/**
 * Hook for managing keyboard shortcuts with condition checks
 * Automatically disables shortcuts when dialogs are open
 */
export function useConditionalKeyboardShortcuts(
  shortcuts: KeyboardShortcut[],
  conditions: {
    hasOpenDialog?: boolean;
    hasOpenPanel?: boolean;
    disabled?: boolean;
  }
) {
  const enabled = !conditions.disabled && !conditions.hasOpenDialog;

  useKeyboardShortcuts(shortcuts, enabled);
}

/**
 * Helper to create keyboard shortcut configuration
 */
export function createShortcut(
  key: string,
  action: () => void,
  modifiers?: {
    ctrl?: boolean;
    alt?: boolean;
    shift?: boolean;
    meta?: boolean;
  },
  description?: string
): KeyboardShortcut {
  return {
    key,
    action,
    modifiers,
    description,
    enabled: true,
  };
}