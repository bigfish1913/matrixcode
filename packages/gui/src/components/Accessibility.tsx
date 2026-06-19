import React from 'react';

// ARIA labels and roles for accessibility

// Message role ARIA mapping
const MESSAGE_ARIA_ROLES: Record<string, string> = {
  user: 'User message',
  assistant: 'AI assistant message',
  tool: 'Tool execution result',
  error: 'Error message',
};

// Generate unique IDs for accessibility
let ariaIdCounter = 0;
export function generateAriaId(prefix: string = 'aria'): string {
  return `${prefix}-${++ariaIdCounter}`;
}

// Accessible message component wrapper
interface AccessibleMessageProps {
  role: 'user' | 'assistant' | 'tool' | 'error';
  children: React.ReactNode;
  ariaLabel?: string;
  ariaDescribedBy?: string;
  isExpanded?: boolean;
  onToggleExpand?: () => void;
}

export function AccessibleMessage({
  role,
  children,
  ariaLabel,
  ariaDescribedBy,
  isExpanded,
  onToggleExpand,
}: AccessibleMessageProps) {
  const ariaRole = MESSAGE_ARIA_ROLES[role];
  const messageId = generateAriaId('msg');
  const descriptionId = generateAriaId('desc');

  return (
    <div
      role="article"
      aria-label={ariaLabel || ariaRole}
      aria-describedby={ariaDescribedBy || descriptionId}
      aria-expanded={isExpanded !== undefined ? isExpanded : undefined}
      id={messageId}
      className="accessible-message"
    >
      {/* Hidden description for screen readers */}
      <span
        id={descriptionId}
        className="sr-only"
        aria-hidden="true"
      >
        {ariaRole}
      </span>

      {/* Content */}
      {children}

      {/* Expand/collapse button for long messages */}
      {onToggleExpand && (
        <button
          onClick={onToggleExpand}
          aria-expanded={isExpanded}
          aria-controls={messageId}
          className="sr-only focus:not-sr-only focus:absolute focus:bottom-0 focus:right-0 focus:bg-background focus:px-2 focus:py-1 focus:text-xs"
        >
          {isExpanded ? 'Collapse message' : 'Expand message'}
        </button>
      )}
    </div>
  );
}

// Accessible button with keyboard support
interface AccessibleButtonProps {
  children: React.ReactNode;
  onClick?: () => void;
  ariaLabel: string;
  ariaPressed?: boolean;
  ariaExpanded?: boolean;
  ariaHasPopup?: boolean;
  disabled?: boolean;
  className?: string;
  shortcut?: string;  // Keyboard shortcut hint
}

export function AccessibleButton({
  children,
  onClick,
  ariaLabel,
  ariaPressed,
  ariaExpanded,
  ariaHasPopup,
  disabled = false,
  className = '',
  shortcut,
}: AccessibleButtonProps) {
  return (
    <button
      onClick={onClick}
      aria-label={ariaLabel}
      aria-pressed={ariaPressed}
      aria-expanded={ariaExpanded}
      aria-haspopup={ariaHasPopup}
      disabled={disabled}
      className={`${className} focus:ring-2 focus:ring-primary focus:ring-offset-2 focus:outline-none`}
    >
      {children}
      {/* Keyboard shortcut hint */}
      {shortcut && (
        <span className="sr-only">
          Press {shortcut} to activate
        </span>
      )}
    </button>
  );
}

// Accessible dialog component
interface AccessibleDialogProps {
  children: React.ReactNode;
  isOpen: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  className?: string;
}

export function AccessibleDialog({
  children,
  isOpen,
  onClose,
  title,
  description,
  className = '',
}: AccessibleDialogProps) {
  const titleId = generateAriaId('dialog-title');
  const descriptionId = generateAriaId('dialog-desc');

  if (!isOpen) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      aria-describedby={description ? descriptionId : undefined}
      className={className}
    >
      {/* Title */}
      <h2 id={titleId} className="sr-only">
        {title}
      </h2>

      {/* Description */}
      {description && (
        <p id={descriptionId} className="sr-only">
          {description}
        </p>
      )}

      {/* Content */}
      {children}

      {/* Close button */}
      <button
        onClick={onClose}
        aria-label="Close dialog"
        className="sr-only focus:not-sr-only focus:absolute focus:top-0 focus:right-0 focus:bg-background focus:px-2 focus:py-1"
      >
        ✕
      </button>
    </div>
  );
}

// Accessible input component
interface AccessibleInputProps {
  value: string;
  onChange: (value: string) => void;
  ariaLabel: string;
  ariaRequired?: boolean;
  ariaInvalid?: boolean;
  ariaErrorMessage?: string;
  placeholder?: string;
  disabled?: boolean;
  maxLength?: number;
  className?: string;
}

export function AccessibleInput({
  value,
  onChange,
  ariaLabel,
  ariaRequired = false,
  ariaInvalid = false,
  ariaErrorMessage,
  placeholder,
  disabled = false,
  maxLength,
  className = '',
}: AccessibleInputProps) {
  const inputId = generateAriaId('input');
  const errorId = generateAriaId('error');

  return (
    <div>
      <input
        id={inputId}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        aria-label={ariaLabel}
        aria-required={ariaRequired}
        aria-invalid={ariaInvalid}
        aria-describedby={ariaInvalid && ariaErrorMessage ? errorId : undefined}
        placeholder={placeholder}
        disabled={disabled}
        maxLength={maxLength}
        className={`${className} focus:ring-2 focus:ring-primary focus:ring-offset-2 focus:outline-none`}
      />
      {/* Error message */}
      {ariaInvalid && ariaErrorMessage && (
        <span
          id={errorId}
          role="alert"
          className="sr-only"
        >
          {ariaErrorMessage}
        </span>
      )}
      {/* Character count hint */}
      {maxLength && (
        <span className="sr-only">
          {value.length} of {maxLength} characters entered
        </span>
      )}
    </div>
  );
}

// Accessible list component
interface AccessibleListProps {
  children: React.ReactNode;
  ariaLabel: string;
  ariaLive?: 'polite' | 'assertive' | 'off';
  className?: string;
}

export function AccessibleList({
  children,
  ariaLabel,
  ariaLive = 'polite',
  className = '',
}: AccessibleListProps) {
  return (
    <div
      role="list"
      aria-label={ariaLabel}
      aria-live={ariaLive}
      className={className}
    >
      {children}
    </div>
  );
}

// Accessible list item
interface AccessibleListItemProps {
  children: React.ReactNode;
  ariaLabel?: string;
  ariaSelected?: boolean;
  ariaCurrent?: boolean;
  onClick?: () => void;
  className?: string;
}

export function AccessibleListItem({
  children,
  ariaLabel,
  ariaSelected,
  ariaCurrent,
  onClick,
  className = '',
}: AccessibleListItemProps) {
  return (
    <div
      role="listitem"
      aria-label={ariaLabel}
      aria-selected={ariaSelected}
      aria-current={ariaCurrent}
      onClick={onClick}
      className={`${className} ${onClick ? 'cursor-pointer' : ''}`}
      tabIndex={onClick ? 0 : undefined}
      onKeyDown={(e) => {
        if (onClick && (e.key === 'Enter' || e.key === ' ')) {
          e.preventDefault();
          onClick();
        }
      }}
    >
      {children}
    </div>
  );
}

// Accessible status indicator
interface AccessibleStatusProps {
  message: string;
  type: 'success' | 'error' | 'warning' | 'info';
  ariaLive?: 'polite' | 'assertive';
}

export function AccessibleStatus({
  message,
  type,
  ariaLive = 'polite',
}: AccessibleStatusProps) {
  const statusId = generateAriaId('status');

  return (
    <div
      id={statusId}
      role="status"
      aria-live={ariaLive}
      className="sr-only"
    >
      {type === 'error' ? `Error: ${message}` : message}
    </div>
  );
}

// Accessible progress indicator
interface AccessibleProgressProps {
  value: number;
  max: number;
  ariaLabel: string;
}

export function AccessibleProgress({
  value,
  max,
  ariaLabel,
}: AccessibleProgressProps) {
  const percentage = Math.round((value / max) * 100);

  return (
    <div
      role="progressbar"
      aria-valuenow={value}
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuetext={`${percentage}% complete`}
      aria-label={ariaLabel}
      className="h-2 bg-muted rounded-full overflow-hidden"
    >
      <div
        className="h-full bg-primary transition-all"
        style={{ width: `${percentage}%` }}
      />
    </div>
  );
}

// Skip to content link (for keyboard navigation)
export function SkipToContent() {
  const contentId = 'main-content';

  return (
    <a
      href={`#${contentId}`}
      className="sr-only focus:not-sr-only focus:absolute focus:top-4 focus:left-4 focus:z-50 focus:bg-background focus:text-foreground focus:px-4 focus:py-2 focus:rounded focus:shadow-lg"
    >
      Skip to main content
    </a>
  );
}

// Screen reader only text
export function SrOnly({ children }: { children: React.ReactNode }) {
  return (
    <span className="sr-only">
      {children}
    </span>
  );
}

// Focus trap component (for modal dialogs)
interface FocusTrapProps {
  children: React.ReactNode;
  active: boolean;
}

export function FocusTrap({ children, active }: FocusTrapProps) {
  const containerRef = React.useRef<HTMLDivElement>(null);
  const previousFocus = React.useRef<HTMLElement | null>(null);

  React.useEffect(() => {
    if (active) {
      // Store previous focus
      previousFocus.current = document.activeElement as HTMLElement;

      // Focus first focusable element in container
      const container = containerRef.current;
      if (container) {
        const focusables = container.querySelectorAll(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        if (focusables.length > 0) {
          (focusables[0] as HTMLElement).focus();
        }
      }
    } else {
      // Restore previous focus
      if (previousFocus.current) {
        previousFocus.current.focus();
      }
    }
  }, [active]);

  // Handle Tab key to trap focus
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!active || e.key !== 'Tab') return;

    const container = containerRef.current;
    if (!container) return;

    const focusables = Array.from(
      container.querySelectorAll(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      )
    ) as HTMLElement[];

    if (focusables.length === 0) return;

    const firstFocusable = focusables[0];
    const lastFocusable = focusables[focusables.length - 1];

    if (e.shiftKey) {
      // Shift+Tab: focus previous or last
      if (document.activeElement === firstFocusable) {
        e.preventDefault();
        lastFocusable.focus();
      }
    } else {
      // Tab: focus next or first
      if (document.activeElement === lastFocusable) {
        e.preventDefault();
        firstFocusable.focus();
      }
    }
  };

  return (
    <div ref={containerRef} onKeyDown={handleKeyDown}>
      {children}
    </div>
  );
}