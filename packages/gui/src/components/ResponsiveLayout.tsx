import React, { useState, useEffect, useCallback } from 'react';

// Screen size breakpoints
export const BREAKPOINTS = {
  xs: 0,      // Extra small (mobile)
  sm: 640,    // Small (tablet portrait)
  md: 768,    // Medium (tablet landscape)
  lg: 1024,   // Large (desktop)
  xl: 1280,   // Extra large (large desktop)
  xxl: 1536,  // Extra extra large (4K)
};

// Screen size type
export type ScreenSize = 'xs' | 'sm' | 'md' | 'lg' | 'xl' | 'xxl';

// Hook for detecting current screen size
export function useScreenSize(): ScreenSize {
  const [screenSize, setScreenSize] = useState<ScreenSize>('lg');

  useEffect(() => {
    const handleResize = () => {
      const width = window.innerWidth;

      if (width < BREAKPOINTS.sm) setScreenSize('xs');
      else if (width < BREAKPOINTS.md) setScreenSize('sm');
      else if (width < BREAKPOINTS.lg) setScreenSize('md');
      else if (width < BREAKPOINTS.xl) setScreenSize('lg');
      else if (width < BREAKPOINTS.xxl) setScreenSize('xl');
      else setScreenSize('xxl');
    };

    handleResize(); // Initial check
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  return screenSize;
}

// Hook for responsive values
export function useResponsive<T>(values: Partial<Record<ScreenSize, T>>): T | undefined {
  const screenSize = useScreenSize();
  const sizes = ['xxl', 'xl', 'lg', 'md', 'sm', 'xs'] as ScreenSize[];

  // Find the first matching size (from large to small)
  for (const size of sizes) {
    if (size === screenSize && values[size] !== undefined) {
      return values[size];
    }
    // Fall back to larger size if current size not defined
    if (values[size] !== undefined) {
      return values[size];
    }
  }

  return undefined;
}

// Hook for detecting mobile
export function useIsMobile(): boolean {
  const screenSize = useScreenSize();
  return screenSize === 'xs' || screenSize === 'sm';
}

// Hook for detecting tablet
export function useIsTablet(): boolean {
  const screenSize = useScreenSize();
  return screenSize === 'md';
}

// Hook for detecting desktop
export function useIsDesktop(): boolean {
  const screenSize = useScreenSize();
  return screenSize === 'lg' || screenSize === 'xl' || screenSize === 'xxl';
}

// Responsive container component
interface ResponsiveContainerProps {
  children: React.ReactNode;
  className?: string;
  maxWidth?: Partial<Record<ScreenSize, string>>;
  padding?: Partial<Record<ScreenSize, string>>;
  hideOn?: ScreenSize[];
  showOn?: ScreenSize[];
}

export function ResponsiveContainer({
  children,
  className = '',
  maxWidth = { xs: '100%', sm: '100%', md: '720px', lg: '960px', xl: '1140px', xxl: '1320px' },
  padding = { xs: 'px-2', sm: 'px-3', md: 'px-4', lg: 'px-6' },
  hideOn = [],
  showOn,
}: ResponsiveContainerProps) {
  const screenSize = useScreenSize();
  const maxWidthValue = useResponsive(maxWidth);
  const paddingValue = useResponsive(padding);

  // Check visibility
  if (showOn && !showOn.includes(screenSize)) {
    return null;
  }
  if (hideOn.includes(screenSize)) {
    return null;
  }

  return (
    <div
      className={`${className} ${paddingValue}`}
      style={{ maxWidth: maxWidthValue, margin: '0 auto' }}
    >
      {children}
    </div>
  );
}

// Responsive sidebar component
interface ResponsiveSidebarProps {
  children: React.ReactNode;
  collapsed?: boolean;
  onToggle?: () => void;
  width?: Partial<Record<ScreenSize, string>>;
}

export function ResponsiveSidebar({
  children,
  collapsed = false,
  onToggle,
  width = { xs: '100%', sm: '280px', md: '240px', lg: '260px' },
}: ResponsiveSidebarProps) {
  const screenSize = useScreenSize();
  const isMobile = useIsMobile();
  const widthValue = useResponsive(width);

  // Mobile: overlay sidebar
  if (isMobile) {
    if (!collapsed) {
      return (
        <>
          {/* Backdrop */}
          <div
            className="fixed inset-0 bg-black/50 z-40"
            onClick={onToggle}
          />
          {/* Sidebar */}
          <div
            className="fixed left-0 top-0 h-full bg-card border-r z-50 shadow-lg"
            style={{ width: widthValue }}
          >
            {children}
          </div>
        </>
      );
    }
    return null;
  }

  // Desktop: fixed sidebar
  return (
    <div
      className={`flex-shrink-0 border-r bg-card transition-all ${
        collapsed ? 'w-0 overflow-hidden' : ''
      }`}
      style={{ width: collapsed ? 0 : widthValue }}
    >
      {!collapsed && children}
    </div>
  );
}

// Responsive grid component
interface ResponsiveGridProps {
  children: React.ReactNode;
  cols?: Partial<Record<ScreenSize, number>>;
  gap?: Partial<Record<ScreenSize, string>>;
  className?: string;
}

export function ResponsiveGrid({
  children,
  cols = { xs: 1, sm: 2, md: 2, lg: 3, xl: 4 },
  gap = { xs: 'gap-2', sm: 'gap-3', md: 'gap-4' },
  className = '',
}: ResponsiveGridProps) {
  const colsValue = useResponsive(cols) || 3;
  const gapValue = useResponsive(gap) || 'gap-4';

  return (
    <div
      className={`grid ${gapValue} ${className}`}
      style={{ gridTemplateColumns: `repeat(${colsValue}, minmax(0, 1fr))` }}
    >
      {children}
    </div>
  );
}

// Responsive text component
interface ResponsiveTextProps {
  children: React.ReactNode;
  size?: Partial<Record<ScreenSize, string>>;
  className?: string;
}

export function ResponsiveText({
  children,
  size = { xs: 'text-sm', sm: 'text-sm', md: 'text-base', lg: 'text-base' },
  className = '',
}: ResponsiveTextProps) {
  const sizeValue = useResponsive(size) || 'text-base';

  return (
    <span className={`${sizeValue} ${className}`}>
      {children}
    </span>
  );
}

// Responsive button component
interface ResponsiveButtonProps {
  children: React.ReactNode;
  onClick?: () => void;
  size?: Partial<Record<ScreenSize, string>>;
  className?: string;
  disabled?: boolean;
}

export function ResponsiveButton({
  children,
  onClick,
  size = { xs: 'sm', sm: 'sm', md: 'md', lg: 'md' },
  className = '',
  disabled = false,
}: ResponsiveButtonProps) {
  const sizeValue = useResponsive(size) || 'md';

  const sizeClasses: Record<string, string> = {
    sm: 'px-3 py-1.5 text-xs',
    md: 'px-4 py-2 text-sm',
    lg: 'px-6 py-3 text-base',
  };

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`${sizeClasses[sizeValue]} ${className}`}
    >
      {children}
    </button>
  );
}

// Screen size indicator (for debugging)
export function ScreenSizeIndicator() {
  const screenSize = useScreenSize();
  const width = window.innerWidth;

  // Check if in development mode (simple check)
  const isDev = window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1';

  if (!isDev) {
    return null;
  }

  return (
    <div className="fixed bottom-4 right-4 bg-black/80 text-white px-2 py-1 rounded text-xs font-mono z-50">
      {screenSize} ({width}px)
    </div>
  );
}