import React, { useRef, useState, useEffect, useCallback, useMemo } from 'react';

interface VirtualScrollProps<T> {
  items: T[];
  itemHeight: number | ((index: number, item: T) => number);
  containerHeight: number;
  renderItem: (item: T, index: number) => React.ReactNode;
  overscan?: number;
  getItemKey?: (item: T, index: number) => string;
  onScroll?: (scrollTop: number) => void;
}

// Virtual scroll implementation for large message lists
export function VirtualScroll<T>({
  items,
  itemHeight,
  containerHeight,
  renderItem,
  overscan = 5,
  getItemKey,
  onScroll,
}: VirtualScrollProps<T>) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);

  // Calculate item heights
  const getItemHeight = useCallback((index: number, item: T): number => {
    if (typeof itemHeight === 'function') {
      return itemHeight(index, item);
    }
    return itemHeight;
  }, [itemHeight]);

  // Calculate total height and item positions
  const { itemPositions, totalHeight } = useMemo(() => {
    const positions: number[] = [];
    let height = 0;

    items.forEach((item, index) => {
      positions.push(height);
      height += getItemHeight(index, item);
    });

    return { itemPositions: positions, totalHeight: height };
  }, [items, getItemHeight]);

  // Find visible range using binary search
  const findVisibleRange = useCallback((scrollTop: number) => {
    const viewportBottom = scrollTop + containerHeight;

    // Find start index (first item that overlaps viewport)
    let startIndex = 0;
    for (let i = 0; i < itemPositions.length; i++) {
      const pos = itemPositions[i];
      const h = getItemHeight(i, items[i]);
      if (pos + h > scrollTop) {
        startIndex = Math.max(0, i - overscan);
        break;
      }
    }

    // Find end index (last item that overlaps viewport)
    let endIndex = items.length - 1;
    for (let i = startIndex; i < itemPositions.length; i++) {
      const pos = itemPositions[i];
      if (pos >= viewportBottom) {
        endIndex = Math.min(items.length - 1, i + overscan);
        break;
      }
    }

    return { startIndex, endIndex };
  }, [items, itemPositions, containerHeight, overscan, getItemHeight]);

  // Get visible range
  const visibleRange = findVisibleRange(scrollTop);

  // Handle scroll event
  const handleScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    const newScrollTop = e.currentTarget.scrollTop;
    setScrollTop(newScrollTop);
    onScroll?.(newScrollTop);
  }, [onScroll]);

  // Render visible items
  return (
    <div
      ref={containerRef}
      onScroll={handleScroll}
      style={{
        height: containerHeight,
        overflow: 'auto',
        position: 'relative',
      }}
      className="scrollbar-thin scrollbar-thumb-muted scrollbar-track-transparent"
    >
      <div style={{ height: totalHeight, position: 'relative' }}>
        {items.slice(visibleRange.startIndex, visibleRange.endIndex + 1).map((item, idx) => {
          const actualIndex = visibleRange.startIndex + idx;
          const key = getItemKey ? getItemKey(item, actualIndex) : `item-${actualIndex}`;
          const position = itemPositions[actualIndex];

          return (
            <div
              key={key}
              style={{
                position: 'absolute',
                top: position,
                width: '100%',
              }}
            >
              {renderItem(item, actualIndex)}
            </div>
          );
        })}
      </div>
    </div>
  );
}