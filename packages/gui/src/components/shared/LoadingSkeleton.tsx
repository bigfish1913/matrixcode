import React from 'react';

/**
 * Skeleton loading components for better perceived performance
 * Replaces simple text fallbacks with animated placeholders
 */

/**
 * Skeleton base component
 */
function Skeleton({ className }: { className?: string }) {
  return (
    <div
      className={`bg-muted animate-pulse rounded ${className || ''}`}
      aria-hidden="true"
    />
  );
}

/**
 * Dialog skeleton - matches dialog structure
 */
export function DialogSkeleton() {
  return (
    <div className="bg-card border shadow-lg rounded-lg max-w-lg w-full overflow-hidden animate-pulse">
      {/* Header */}
      <div className="p-3 border-b">
        <Skeleton className="h-4 w-1/3" />
      </div>

      {/* Body */}
      <div className="p-4 space-y-3">
        <Skeleton className="h-3 w-full" />
        <Skeleton className="h-3 w-4/5" />
        <Skeleton className="h-3 w-2/3" />
        <div className="space-y-2 mt-4">
          <Skeleton className="h-8 w-full" />
          <Skeleton className="h-8 w-full" />
          <Skeleton className="h-8 w-3/4" />
        </div>
      </div>

      {/* Footer */}
      <div className="px-4 py-2 bg-muted/30">
        <Skeleton className="h-3 w-1/2" />
      </div>
    </div>
  );
}

/**
 * Panel skeleton - matches panel structure
 */
export function PanelSkeleton() {
  return (
    <div className="bg-card border shadow-lg rounded-lg p-4 animate-pulse">
      {/* Title */}
      <Skeleton className="h-4 w-1/4 mb-4" />

      {/* Content */}
      <div className="space-y-2">
        <Skeleton className="h-3 w-full" />
        <Skeleton className="h-3 w-3/4" />
        <Skeleton className="h-3 w-1/2" />
      </div>

      {/* Status indicators */}
      <div className="flex gap-2 mt-4">
        <Skeleton className="h-6 w-16 rounded-full" />
        <Skeleton className="h-6 w-16 rounded-full" />
      </div>
    </div>
  );
}

/**
 * Settings panel skeleton
 */
export function SettingsSkeleton() {
  return (
    <div className="p-4 animate-pulse">
      {/* Section headers */}
      <Skeleton className="h-5 w-1/4 mb-4" />

      {/* Settings items */}
      <div className="space-y-4">
        {[1, 2, 3, 4].map(i => (
          <div key={i} className="space-y-2">
            <Skeleton className="h-3 w-1/3" />
            <Skeleton className="h-10 w-full" />
          </div>
        ))}
      </div>

      {/* Actions */}
      <div className="flex gap-2 mt-6">
        <Skeleton className="h-9 w-24" />
        <Skeleton className="h-9 w-20" />
      </div>
    </div>
  );
}

/**
 * Message skeleton - for loading messages
 */
export function MessageSkeleton({ isUser = false }: { isUser?: boolean }) {
  return (
    <div className={`flex ${isUser ? 'justify-end' : 'justify-start'} mb-4 animate-pulse`}>
      <div className={`max-w-[85%] rounded-lg px-4 py-2.5 ${isUser ? 'bg-primary' : 'bg-card border'}`}>
        {/* Thinking block (assistant only) */}
        {!isUser && (
          <div className="mb-2">
            <Skeleton className="h-2 w-16 mb-1" />
            <Skeleton className="h-12 w-full" />
          </div>
        )}

        {/* Content */}
        <div className="space-y-1">
          <Skeleton className={`h-3 ${isUser ? 'w-32' : 'w-full'}`} />
          <Skeleton className={`h-3 ${isUser ? 'w-24' : 'w-4/5'}`} />
        </div>

        {/* Timestamp */}
        <Skeleton className="h-2 w-12 mt-2" />
      </div>
    </div>
  );
}

/**
 * Task list skeleton
 */
export function TaskListSkeleton() {
  return (
    <div className="animate-pulse space-y-3">
      {[1, 2, 3].map(i => (
        <div key={i} className="border rounded-lg p-3">
          <div className="flex items-center justify-between mb-2">
            <Skeleton className="h-4 w-1/3" />
            <Skeleton className="h-6 w-16 rounded-full" />
          </div>
          <Skeleton className="h-3 w-full mb-1" />
          <Skeleton className="h-3 w-2/3" />
        </div>
      ))}
    </div>
  );
}

/**
 * Command bar skeleton
 */
export function CommandBarSkeleton() {
  return (
    <div className="bg-card border shadow-lg rounded-lg max-w-lg w-full overflow-hidden animate-pulse">
      <div className="p-3 border-b">
        <Skeleton className="h-4 w-full" />
      </div>
      <div className="max-h-[300px] p-2 space-y-2">
        {[1, 2, 3, 4, 5].map(i => (
          <Skeleton key={i} className="h-8 w-full" />
        ))}
      </div>
    </div>
  );
}

/**
 * Generic inline skeleton for small elements
 */
export function InlineSkeleton({ width = 'full' }: { width?: string }) {
  return <Skeleton className={`h-4 w-${width}`} />;
}

/**
 * Loading message with optional text
 */
export function LoadingMessage({ message = 'Loading...' }: { message?: string }) {
  return (
    <div className="flex items-center gap-2 p-4 text-muted-foreground animate-fade-in">
      <div className="flex gap-1">
        <span className="w-2 h-2 rounded-full bg-primary animate-bounce [animation-delay:-0.3s]" />
        <span className="w-2 h-2 rounded-full bg-primary animate-bounce [animation-delay:-0.15s]" />
        <span className="w-2 h-2 rounded-full bg-primary animate-bounce" />
      </div>
      <span className="text-sm">{message}</span>
    </div>
  );
}