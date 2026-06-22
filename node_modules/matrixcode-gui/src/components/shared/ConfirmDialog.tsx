import React from 'react';

/**
 * Confirmation dialog for destructive actions
 * Prevents accidental data loss and improves user confidence
 */

interface ConfirmDialogProps {
  title: string;
  message: string;
  onConfirm: () => void;
  onCancel: () => void;
  confirmText?: string;
  cancelText?: string;
  variant?: 'warning' | 'danger' | 'info';
}

export function ConfirmDialog({
  title,
  message,
  onConfirm,
  onCancel,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  variant = 'warning',
}: ConfirmDialogProps) {
  const variantConfig = {
    warning: {
      icon: '⚠️',
      confirmBg: 'bg-yellow-500 hover:bg-yellow-600',
      borderColor: 'border-yellow-200',
    },
    danger: {
      icon: '🗑️',
      confirmBg: 'bg-red-500 hover:bg-red-600',
      borderColor: 'border-red-200',
    },
    info: {
      icon: 'ℹ️',
      confirmBg: 'bg-blue-500 hover:bg-blue-600',
      borderColor: 'border-blue-200',
    },
  };

  const config = variantConfig[variant];

  return (
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 animate-fade-in"
      onClick={(e) => {
        if (e.target === e.currentTarget) {
          onCancel();
        }
      }}
    >
      <div
        className="bg-card border shadow-lg rounded-lg max-w-sm w-full p-4 animate-slide-in-up"
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        aria-describedby="confirm-message"
      >
        {/* Header */}
        <div className="flex items-center gap-3 mb-3">
          <span className="text-2xl">{config.icon}</span>
          <h3
            id="confirm-title"
            className="font-semibold text-lg"
          >
            {title}
          </h3>
        </div>

        {/* Message */}
        <p
          id="confirm-message"
          className="text-sm text-muted-foreground mb-4"
        >
          {message}
        </p>

        {/* Actions */}
        <div className="flex gap-2 justify-end">
          <button
            onClick={onCancel}
            className="px-4 py-2 bg-muted rounded hover:bg-muted/80 transition-colors focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-muted"
            autoFocus
          >
            {cancelText}
          </button>
          <button
            onClick={onConfirm}
            className={`px-4 py-2 ${config.confirmBg} text-white rounded transition-colors focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-red-500`}
          >
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * Quick confirm for simple actions
 * Returns promise that resolves to boolean
 */
export function showConfirmDialog(
  title: string,
  message: string,
  variant?: 'warning' | 'danger' | 'info'
): Promise<boolean> {
  return new Promise((resolve) => {
    const container = document.createElement('div');
    document.body.appendChild(container);

    const handleConfirm = () => {
      resolve(true);
      cleanup();
    };

    const handleCancel = () => {
      resolve(false);
      cleanup();
    };

    const cleanup = () => {
      // Use React's unmountComponentAtNode equivalent
      // For now, just remove the container
      setTimeout(() => {
        document.body.removeChild(container);
      }, 300); // Wait for animation
    };

    // Note: This would need React 18's createRoot or ReactDOM.render
    // For now, just showing the pattern
    console.log('showConfirmDialog would render:', { title, message, variant });
  });
}

/**
 * Hook for managing confirm dialog state
 */
export function useConfirmDialog() {
  const [state, setState] = React.useState<{
    visible: boolean;
    config: {
      title: string;
      message: string;
      onConfirm: () => void;
      variant?: 'warning' | 'danger' | 'info';
    } | null;
  }>({
    visible: false,
    config: null,
  });

  const showConfirm = React.useCallback((
    title: string,
    message: string,
    onConfirm: () => void,
    variant?: 'warning' | 'danger' | 'info'
  ) => {
    setState({
      visible: true,
      config: { title, message, onConfirm, variant },
    });
  }, []);

  const hideConfirm = React.useCallback(() => {
    setState({ visible: false, config: null });
  }, []);

  const handleConfirm = React.useCallback(() => {
    if (state.config?.onConfirm) {
      state.config.onConfirm();
    }
    hideConfirm();
  }, [state.config, hideConfirm]);

  return {
    visible: state.visible,
    config: state.config,
    showConfirm,
    hideConfirm,
    handleConfirm,
    handleCancel: hideConfirm,
  };
}