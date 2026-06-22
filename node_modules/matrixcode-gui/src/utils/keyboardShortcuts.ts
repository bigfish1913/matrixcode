import React, { useEffect, useState, useCallback } from 'react';

// Keyboard shortcut definition (matching TUI key handling)
interface KeyboardShortcut {
  key: string;
  modifiers?: {
    ctrl?: boolean;
    alt?: boolean;
    shift?: boolean;
    meta?: boolean;
  };
  description: string;
  category: string;
  action?: () => void;
}

// Global keyboard shortcuts registry (matching TUI shortcuts)
export const KEYBOARD_SHORTCUTS: KeyboardShortcut[] = [
  // Workflow and Panels
  { key: 'w', modifiers: { alt: true }, description: 'Toggle workflow panel', category: 'Panels' },
  { key: 'm', modifiers: { alt: true }, description: 'Toggle approve mode', category: 'Mode' },
  { key: 'Tab', modifiers: { shift: true }, description: 'Toggle approve mode', category: 'Mode' },
  { key: 'l', modifiers: { alt: true }, description: 'LSP server status', category: 'Panels' },
  { key: 'g', modifiers: { alt: true }, description: 'CodeGraph status', category: 'Panels' },
  { key: 'D', modifiers: { shift: true }, description: 'Toggle debug panel', category: 'Panels' },
  { key: 'P', modifiers: { shift: true }, description: 'Toggle performance monitor', category: 'Panels' },

  // Navigation
  { key: 'Home', description: 'Scroll to top', category: 'Navigation' },
  { key: 'End', description: 'Scroll to bottom', category: 'Navigation' },
  { key: 'PageUp', description: 'Scroll up one page', category: 'Navigation' },
  { key: 'PageDown', description: 'Scroll down one page', category: 'Navigation' },

  // Commands
  { key: '/', description: 'Open command bar', category: 'Commands' },
  { key: '?', modifiers: { shift: true }, description: 'Show shortcut help', category: 'Commands' },

  // Actions
  { key: 'Escape', description: 'Stop agent / Cancel operation', category: 'Actions' },
  { key: 'Escape', modifiers: { shift: true }, description: 'Remove first pending message', category: 'Actions' },

  // Input
  { key: 'Enter', description: 'Send message', category: 'Input' },
  { key: 'Enter', modifiers: { shift: true }, description: 'Insert newline', category: 'Input' },
  { key: 'k', modifiers: { ctrl: true }, description: 'Clear input', category: 'Input' },
  { key: 'ArrowUp', description: 'Navigate input history (at start)', category: 'Input' },
  { key: 'ArrowDown', description: 'Navigate input history (at end)', category: 'Input' },
];

// Keyboard shortcut manager hook
export function useKeyboardShortcuts(
  handlers: Record<string, () => void>
) {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Check each shortcut
      for (const shortcut of KEYBOARD_SHORTCUTS) {
        // Check key match
        if (e.key !== shortcut.key && e.key.toLowerCase() !== shortcut.key.toLowerCase()) {
          continue;
        }

        // Check modifiers match
        const mods = shortcut.modifiers || {};
        const ctrlMatch = mods.ctrl ? (e.ctrlKey || e.metaKey) : !e.ctrlKey && !e.metaKey;
        const altMatch = mods.alt ? e.altKey : !e.altKey;
        const shiftMatch = mods.shift ? e.shiftKey : !e.shiftKey;

        if (!ctrlMatch || !altMatch || !shiftMatch) {
          continue;
        }

        // Find matching handler
        const handlerKey = [
          shortcut.key.toLowerCase(),
          mods.ctrl ? 'ctrl' : '',
          mods.alt ? 'alt' : '',
          mods.shift ? 'shift' : '',
        ].filter(Boolean).join('+');

        const handler = handlers[handlerKey] || handlers[shortcut.key.toLowerCase()];

        if (handler) {
          e.preventDefault();
          handler();
          break;
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handlers]);
}

// Keyboard shortcut display component (for help dialog) - moved to separate .tsx file
export function formatShortcutList(shortcuts = KEYBOARD_SHORTCUTS): Array<{ category: string; items: Array<{ description: string; keyCombo: string }> }> {
  const groupedShortcuts = shortcuts.reduce((acc, shortcut) => {
    const category = shortcut.category;
    if (!acc[category]) acc[category] = [];
    acc[category].push(shortcut);
    return acc;
  }, {} as Record<string, KeyboardShortcut[]>);

  return Object.entries(groupedShortcuts).map(([category, shortcuts]) => ({
    category,
    items: shortcuts.map(shortcut => ({
      description: shortcut.description,
      keyCombo: formatShortcut(shortcut),
    })),
  }));
}

// Format shortcut for display
export function formatShortcut(shortcut: KeyboardShortcut): string {
  const parts: string[] = [];
  if (shortcut.modifiers?.ctrl) parts.push('Ctrl/Cmd');
  if (shortcut.modifiers?.alt) parts.push('Alt');
  if (shortcut.modifiers?.shift) parts.push('Shift');
  parts.push(shortcut.key);
  return parts.join('+');
}

// Keyboard shortcut checker utility
export function checkShortcut(e: KeyboardEvent, shortcut: KeyboardShortcut): boolean {
  const mods = shortcut.modifiers || {};

  const keyMatch = e.key === shortcut.key || e.key.toLowerCase() === shortcut.key.toLowerCase();
  const ctrlMatch = mods.ctrl ? (e.ctrlKey || e.metaKey) : !e.ctrlKey && !e.metaKey;
  const altMatch = mods.alt ? e.altKey : !e.altKey;
  const shiftMatch = mods.shift ? e.shiftKey : !e.shiftKey;

  return keyMatch && ctrlMatch && altMatch && shiftMatch;
}