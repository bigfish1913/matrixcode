import React, { Component, ErrorInfo, ReactNode } from 'react';

/**
 * Error boundary component for graceful error handling
 * Prevents entire app crash when component errors occur
 */

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
  onError?: (error: Error, errorInfo: ErrorInfo) => void;
}

interface State {
  hasError: boolean;
  error?: Error;
}

export class ErrorBoundary extends Component<Props, State> {
  public state: State = { hasError: false };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    // Log error for debugging
    console.error('Error caught by boundary:', error, errorInfo);

    // Call optional error handler
    if (this.props.onError) {
      this.props.onError(error, errorInfo);
    }
  }

  public render() {
    if (this.state.hasError) {
      // Use custom fallback if provided
      if (this.props.fallback) {
        return this.props.fallback;
      }

      // Default error UI
      return (
        <div className="p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg animate-fade-in">
          <div className="flex items-start gap-3">
            <span className="text-2xl">⚠️</span>
            <div className="flex-1">
              <h2 className="text-red-800 dark:text-red-200 font-semibold mb-2">
                Something went wrong
              </h2>
              <p className="text-red-600 dark:text-red-300 text-sm mb-3">
                {this.state.error?.message || 'An unexpected error occurred'}
              </p>
              <div className="flex gap-2">
                <button
                  onClick={() => this.setState({ hasError: false })}
                  className="px-3 py-1.5 bg-red-100 dark:bg-red-800 hover:bg-red-200 dark:hover:bg-red-700 rounded text-red-800 dark:text-red-200 transition-colors"
                >
                  Try again
                </button>
                <button
                  onClick={() => window.location.reload()}
                  className="px-3 py-1.5 bg-muted hover:bg-muted/80 rounded text-muted-foreground hover:text-foreground transition-colors"
                >
                  Reload page
                </button>
              </div>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

/**
 * Specialized error boundary for dialogs
 * Shows minimal error UI suitable for overlay contexts
 */
export class DialogErrorBoundary extends Component<Props, State> {
  public state: State = { hasError: false };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Dialog error:', error, errorInfo);
    if (this.props.onError) {
      this.props.onError(error, errorInfo);
    }
  }

  public render() {
    if (this.state.hasError) {
      return (
        <div className="bg-card border shadow-lg rounded-lg max-w-lg w-full p-4 animate-fade-in">
          <div className="flex items-center gap-2 mb-3">
            <span className="text-xl">⚠️</span>
            <h3 className="font-semibold text-red-600">Error loading dialog</h3>
          </div>
          <p className="text-sm text-muted-foreground mb-3">
            {this.state.error?.message}
          </p>
          <button
            onClick={() => this.setState({ hasError: false })}
            className="px-3 py-1.5 bg-muted hover:bg-muted/80 rounded text-sm"
          >
            Retry
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}

/**
 * Error boundary for panel components
 * Inline error display for status panels
 */
export class PanelErrorBoundary extends Component<Props, State> {
  public state: State = { hasError: false };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Panel error:', error, errorInfo);
    if (this.props.onError) {
      this.props.onError(error, errorInfo);
    }
  }

  public render() {
    if (this.state.hasError) {
      return (
        <div className="bg-card border shadow-lg rounded-lg p-3 animate-fade-in">
          <div className="flex items-center gap-2 text-red-500">
            <span>⚠️</span>
            <span className="text-sm font-medium">Error loading panel</span>
          </div>
          <p className="text-xs text-muted-foreground mt-1">
            {this.state.error?.message}
          </p>
        </div>
      );
    }

    return this.props.children;
  }
}