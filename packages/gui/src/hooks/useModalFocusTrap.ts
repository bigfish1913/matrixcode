import { useEffect, useRef } from 'react';

/**
 * Shared hook for modal focus trap and keyboard handling
 *
 * Handles:
 * - Focus trap (Tab key cycling within modal)
 * - Escape key to close
 * - Previous focus restoration on close
 *
 * Usage:
 * ```tsx
 * const modalRef = useRef<HTMLDivElement>(null);
 * useModalFocusTrap(modalRef, onClose, {
 *   onEscape: true,    // Handle Escape key
 *   autoFocus: true,   // Auto-focus first element on mount
 * });
 * ```
 */
interface ModalFocusTrapOptions {
  /** Handle Escape key to close (default: true) */
  onEscape?: boolean;
  /** Auto-focus first focusable element on mount (default: true) */
  autoFocus?: boolean;
  /** Additional keyboard handlers */
  additionalHandlers?: Record<string, (e: KeyboardEvent) => void>;
}

/**
 * Hook for modal dialog focus trap and keyboard management
 *
 * @param modalRef - Ref to the modal container element
 * @param onClose - Callback to close the modal
 * @param options - Configuration options
 */
export function useModalFocusTrap(
  modalRef: React.RefObject<HTMLDivElement>,
  onClose: () => void,
  options: ModalFocusTrapOptions = {}
) {
  const {
    onEscape = true,
    autoFocus = true,
    additionalHandlers = {},
  } = options;

  const prevFocusRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    // Store the previously focused element
    prevFocusRef.current = document.activeElement as HTMLElement;

    // Auto-focus first focusable element if enabled
    if (autoFocus && modalRef.current) {
      const firstFocusable = modalRef.current.querySelector<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      firstFocusable?.focus();
    }

    // Keyboard event handler
    const handleKeyDown = (e: KeyboardEvent) => {
      // Handle Escape key
      if (onEscape && e.key === 'Escape') {
        e.preventDefault();
        onClose();
        return;
      }

      // Handle additional keyboard shortcuts (ArrowUp/Down/Enter, etc.)
      if (additionalHandlers[e.key]) {
        additionalHandlers[e.key](e);
        return;
      }

      // Handle Tab key for focus trap
      if (e.key === 'Tab') {
        const modal = modalRef.current;
        if (!modal) return;

        // Get all focusable elements within the modal
        const focusableElements = modal.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        if (focusableElements.length === 0) return;

        const firstElement = focusableElements[0];
        const lastElement = focusableElements[focusableElements.length - 1];

        if (e.shiftKey) {
          // Shift+Tab: if on first element, move to last
          if (document.activeElement === firstElement) {
            e.preventDefault();
            lastElement?.focus();
          }
        } else {
          // Tab: if on last element, move to first
          if (document.activeElement === lastElement) {
            e.preventDefault();
            firstElement?.focus();
          }
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      // Restore focus to the previous element on close
      prevFocusRef.current?.focus();
    };
  }, [onClose, onEscape, autoFocus, additionalHandlers, modalRef]);

  return prevFocusRef;
}

export default useModalFocusTrap;