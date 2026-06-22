import React, { useEffect, useRef, useState } from 'react';

// Focus zones for Tab navigation (matching TUI layout)
const FOCUS_ZONES = [
  'sidebar',       // Session list
  'messages',      // Message area
  'input',         // Input field
  'actions',       // Action buttons (Send, Stop)
  'debug',         // Debug panel (if open)
  'workflow',      // Workflow panel (if open)
] as const;

type FocusZone = typeof FOCUS_ZONES[number];

interface FocusManagerProps {
  children: React.ReactNode;
}

// Global focus state
let currentFocusZone: FocusZone = 'input';
const focusListeners: Set<(zone: FocusZone) => void> = new Set();

// Navigate to next/previous zone
export function navigateFocus(direction: 'next' | 'prev') {
  const currentIndex = FOCUS_ZONES.indexOf(currentFocusZone);
  let newIndex;

  if (direction === 'next') {
    newIndex = (currentIndex + 1) % FOCUS_ZONES.length;
  } else {
    newIndex = (currentIndex - 1 + FOCUS_ZONES.length) % FOCUS_ZONES.length;
  }

  currentFocusZone = FOCUS_ZONES[newIndex];
  focusListeners.forEach(listener => listener(currentFocusZone));

  // Focus the element in the zone
  const element = document.querySelector(`[data-focus-zone="${currentFocusZone}"]`);
  if (element) {
    (element as HTMLElement).focus();
  }
}

// Set focus zone directly
export function setFocusZone(zone: FocusZone) {
  currentFocusZone = zone;
  focusListeners.forEach(listener => listener(zone));
}

// Get current focus zone
export function getFocusZone(): FocusZone {
  return currentFocusZone;
}

// Focus indicator component
export function FocusIndicator({ zone }: { zone: FocusZone }) {
  const [isFocused, setIsFocused] = useState(currentFocusZone === zone);

  useEffect(() => {
    const listener = (newZone: FocusZone) => {
      setIsFocused(newZone === zone);
    };
    focusListeners.add(listener);
    return () => {
      focusListeners.delete(listener);
    };
  }, [zone]);

  if (!isFocused) return null;

  return (
    <div className="absolute inset-0 ring-2 ring-primary ring-offset-2 ring-offset-background pointer-events-none z-50" />
  );
}

// Focusable wrapper component
export function FocusableZone({
  zone,
  children,
  className = '',
  onFocus,
  onBlur,
}: {
  zone: FocusZone;
  children: React.ReactNode;
  className?: string;
  onFocus?: () => void;
  onBlur?: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);

  const handleFocus = () => {
    setFocusZone(zone);
    onFocus?.();
  };

  const handleBlur = () => {
    onBlur?.();
  };

  // Handle Tab key for navigation
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Tab') {
      e.preventDefault();
      navigateFocus(e.shiftKey ? 'prev' : 'next');
    }
  };

  return (
    <div
      ref={ref}
      data-focus-zone={zone}
      tabIndex={0}
      onFocus={handleFocus}
      onBlur={handleBlur}
      onKeyDown={handleKeyDown}
      className={`relative focus:outline-none ${className}`}
    >
      <FocusIndicator zone={zone} />
      {children}
    </div>
  );
}

// Focus hint bar component
export function FocusHintBar() {
  const [currentZone, setCurrentZone] = useState<FocusZone>(currentFocusZone);

  useEffect(() => {
    const listener = (zone: FocusZone) => {
      setCurrentZone(zone);
    };
    focusListeners.add(listener);
    return () => {
      focusListeners.delete(listener);
    };
  }, []);

  const zoneLabels: Record<FocusZone, string> = {
    sidebar: '侧边栏',
    messages: '消息区',
    input: '输入框',
    actions: '操作按钮',
    debug: '调试面板',
    workflow: '工作流面板',
  };

  return (
    <div className="px-4 py-1 bg-muted/20 text-xs text-muted-foreground flex items-center gap-2">
      <span className="flex items-center gap-1">
        <kbd className="px-1.5 py-0.5 bg-muted rounded">Tab</kbd>
        <span>切换焦点</span>
      </span>
      <span className="text-muted-foreground/50">|</span>
      <span className="flex items-center gap-1">
        <span>当前:</span>
        <span className="text-primary">{zoneLabels[currentZone]}</span>
      </span>
      <span className="text-muted-foreground/50">|</span>
      <span className="flex items-center gap-1">
        {FOCUS_ZONES.map((zone, idx) => (
          <span
            key={zone}
            className={`px-1.5 py-0.5 rounded ${
              zone === currentZone ? 'bg-primary/20 text-primary' : 'bg-muted/50'
            }`}
          >
            {idx + 1}
          </span>
        ))}
      </span>
    </div>
  );
}

// Main focus manager component (keyboard shortcuts)
export function FocusManager({ children }: FocusManagerProps) {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Tab navigation (shift for reverse)
      if (e.key === 'Tab' && !e.ctrlKey && !e.metaKey && !e.altKey) {
        // Let FocusableZone handle Tab navigation
        return;
      }

      // Number keys for quick zone switch (1-6)
      const zoneIndex = parseInt(e.key) - 1;
      if (zoneIndex >= 0 && zoneIndex < FOCUS_ZONES.length) {
        e.preventDefault();
        setFocusZone(FOCUS_ZONES[zoneIndex]);
        const element = document.querySelector(`[data-focus-zone="${FOCUS_ZONES[zoneIndex]}"]`);
        if (element) {
          (element as HTMLElement).focus();
        }
      }

      // F6 for next zone (browser convention)
      if (e.key === 'F6') {
        e.preventDefault();
        navigateFocus('next');
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  return <>{children}</>;
}