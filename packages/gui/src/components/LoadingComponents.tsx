import React, { useState, useEffect } from 'react';

// Enhanced loading fallback with better animations
interface LoadingFallbackProps {
  message?: string;
  type?: 'dialog' | 'panel' | 'component';
}

export function LoadingFallback({
  message = '加载中...',
  type = 'dialog'
}: LoadingFallbackProps) {
  // Different styles based on type
  const containerClass = type === 'dialog'
    ? 'fixed inset-0 bg-black/50 flex items-center justify-center z-50'
    : type === 'panel'
      ? 'flex items-center justify-center p-4'
      : 'flex items-center gap-2';

  const cardClass = type === 'dialog'
    ? 'bg-card border shadow-lg rounded-lg p-6'
    : type === 'panel'
      ? 'bg-card border rounded-lg p-4'
      : '';

  return (
    <div className={`${containerClass} animate-fade-in`}>
      <div className={`${cardClass} animate-pulse`}>
        {/* Spinner animation */}
        <div className="flex items-center gap-3">
          <div className="relative">
            {/* Outer ring */}
            <div className="w-8 h-8 border-2 border-muted rounded-full animate-spin"
                 style={{ borderTopColor: 'var(--primary)' }} />
            {/* Inner dot */}
            <div className="absolute inset-0 flex items-center justify-center">
              <div className="w-2 h-2 bg-primary rounded-full animate-ping" />
            </div>
          </div>

          {/* Loading text */}
          <div className="flex flex-col gap-1">
            <span className="text-sm font-medium text-foreground">{message}</span>
            {type === 'dialog' && (
              <span className="text-xs text-muted-foreground">
                正在加载组件...
              </span>
            )}
          </div>
        </div>

        {/* Progress dots animation */}
        {type === 'dialog' && (
          <div className="flex gap-1 mt-3 justify-center">
            <div className="w-1.5 h-1.5 bg-primary rounded-full animate-bounce [animation-delay:-0.3s]" />
            <div className="w-1.5 h-1.5 bg-primary rounded-full animate-bounce [animation-delay:-0.15s]" />
            <div className="w-1.5 h-1.5 bg-primary rounded-full animate-bounce" />
          </div>
        )}
      </div>
    </div>
  );
}

// Skeleton loader for content placeholders
export function SkeletonLoader({ lines = 3 }: { lines?: number }) {
  return (
    <div className="space-y-2 animate-pulse">
      {Array.from({ length: lines }).map((_, i) => (
        <div
          key={i}
          className="h-4 bg-muted rounded"
          style={{
            width: `${Math.random() * 40 + 60}%`,
            animationDelay: `${i * 0.1}s`
          }}
        />
      ))}
    </div>
  );
}

// Card skeleton for loading panels
export function CardSkeleton() {
  return (
    <div className="bg-card border rounded-lg p-4 animate-pulse">
      <div className="flex items-center gap-3 mb-3">
        <div className="w-10 h-10 bg-muted rounded-full" />
        <div className="flex-1">
          <div className="h-4 bg-muted rounded w-3/4 mb-2" />
          <div className="h-3 bg-muted rounded w-1/2" />
        </div>
      </div>
      <SkeletonLoader lines={2} />
    </div>
  );
}

// Inline loading spinner for small components
export function InlineLoader({ size = 'sm' }: { size?: 'sm' | 'md' | 'lg' }) {
  const sizeClass = {
    sm: 'w-4 h-4',
    md: 'w-6 h-6',
    lg: 'w-8 h-8'
  };

  return (
    <div className={`${sizeClass[size]} border-2 border-muted rounded-full animate-spin`}
         style={{ borderTopColor: 'var(--primary)' }} />
  );
}