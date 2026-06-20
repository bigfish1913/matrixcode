import React from 'react';

/**
 * Keyboard shortcut hint component
 * Improves discoverability of shortcuts
 */

interface ShortcutHintProps {
  shortcut: string;
  description: string;
  className?: string;
}

export function ShortcutHint({
  shortcut,
  description,
  className = '',
}: ShortcutHintProps) {
  return (
    <div className={`flex items-center gap-2 text-xs text-muted-foreground ${className}`}>
      <kbd className="px-1.5 py-0.5 bg-muted border border-border rounded font-mono text-xs">
        {shortcut}
      </kbd>
      <span>{description}</span>
    </div>
  );
}

/**
 * Shortcut group for displaying multiple shortcuts
 */
export function ShortcutGroup({
  shortcuts,
  className = '',
}: {
  shortcuts: Array<{ shortcut: string; description: string }>;
  className?: string;
}) {
  return (
    <div className={`flex flex-wrap gap-3 ${className}`}>
      {shortcuts.map(({ shortcut, description }) => (
        <ShortcutHint
          key={shortcut}
          shortcut={shortcut}
          description={description}
        />
      ))}
    </div>
  );
}

/**
 * Floating shortcut hint indicator
 * Shows at bottom of screen to remind users shortcuts exist
 */
export function ShortcutHintIndicator({
  onClick,
  className = '',
}: {
  onClick: () => void;
  className?: string;
}) {
  return (
    <div className={`fixed bottom-4 right-4 z-30 ${className}`}>
      <button
        onClick={onClick}
        className="px-3 py-1.5 bg-muted/80 hover:bg-muted rounded text-xs transition-all opacity-50 hover:opacity-100 focus:opacity-100 focus:outline-none focus:ring-2 focus:ring-primary/50"
        aria-label="Show keyboard shortcuts"
      >
        Press{' '}
        <kbd className="px-1 bg-background border border-border rounded font-mono">
          ?
        </kbd>
        {' '}for shortcuts
      </button>
    </div>
  );
}

/**
 * Input area shortcut hints
 * Shows shortcuts relevant to input field
 */
export function InputShortcuts() {
  return (
    <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
      <ShortcutHint shortcut="Enter" description="Send" />
      <ShortcutHint shortcut="Shift+Enter" description="New line" />
      <ShortcutHint shortcut="↑↓" description="History" />
      <ShortcutHint shortcut="Esc" description="Stop/Clear" />
    </div>
  );
}

/**
 * Global shortcuts panel
 * Shows important global shortcuts
 */
export function GlobalShortcuts() {
  return (
    <div className="space-y-2">
      <div className="text-xs font-semibold text-foreground mb-2">
        Global Shortcuts
      </div>
      <ShortcutGroup
        shortcuts={[
          { shortcut: '/', description: 'Command bar' },
          { shortcut: '?', description: 'Shortcuts help' },
          { shortcut: 'Ctrl+N', description: 'New chat' },
          { shortcut: 'Ctrl+T', description: 'Tasks' },
          { shortcut: 'Ctrl+,', description: 'Settings' },
        ]}
      />
    </div>
  );
}

/**
 * Panel shortcuts section
 */
export function PanelShortcuts() {
  return (
    <div className="space-y-2">
      <div className="text-xs font-semibold text-foreground mb-2">
        Panel Shortcuts
      </div>
      <ShortcutGroup
        shortcuts={[
          { shortcut: 'Alt+L', description: 'LSP panel' },
          { shortcut: 'Alt+G', description: 'CodeGraph' },
          { shortcut: 'Alt+W', description: 'MCP panel' },
          { shortcut: 'Shift+D', description: 'Debug' },
        ]}
      />
    </div>
  );
}

/**
 * Message shortcuts section
 */
export function MessageShortcuts() {
  return (
    <div className="space-y-2">
      <div className="text-xs font-semibold text-foreground mb-2">
        Message Actions
      </div>
      <ShortcutGroup
        shortcuts={[
          { shortcut: 'Alt+T', description: 'Toggle thinking' },
          { shortcut: 'Alt+↑↓', description: 'Fine scroll' },
          { shortcut: 'Ctrl+C', description: 'Copy/Interrupt' },
          { shortcut: 'Ctrl+R', description: 'Retry' },
        ]}
      />
    </div>
  );
}

/**
 * Complete shortcuts reference for help dialog
 */
export function ShortcutsReference() {
  return (
    <div className="space-y-4 p-4">
      <GlobalShortcuts />
      <PanelShortcuts />
      <MessageShortcuts />

      <div className="border-t pt-4">
        <div className="text-xs font-semibold text-foreground mb-2">
          Quick Actions
        </div>
        <ShortcutGroup
          shortcuts={[
            { shortcut: 'Alt+E', description: 'Explain code' },
            { shortcut: 'Alt+F', description: 'Fix code' },
            { shortcut: 'Alt+T', description: 'Generate tests' },
            { shortcut: 'Alt+R', description: 'Refactor' },
            { shortcut: 'Alt+I', description: 'Improve' },
          ]}
        />
      </div>

      <div className="border-t pt-4">
        <div className="text-xs font-semibold text-foreground mb-2">
          Navigation
        </div>
        <ShortcutGroup
          shortcuts={[
            { shortcut: 'Home', description: 'Scroll to top' },
            { shortcut: 'End', description: 'Scroll to bottom' },
            { shortcut: 'PgUp/PgDn', description: 'Page scroll' },
          ]}
        />
      </div>
    </div>
  );
}