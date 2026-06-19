import React, { useEffect, useState } from 'react';

// Animation variants following TUI style

// Spinner animation frames (matching TUI)
const SPINNER_FRAMES = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

// Spinner component with configurable speed
export function Spinner({ speed = 80 }: { speed?: number }) {
  const [frame, setFrame] = useState(0);

  useEffect(() => {
    const interval = setInterval(() => {
      setFrame((f) => (f + 1) % SPINNER_FRAMES.length);
    }, speed);
    return () => clearInterval(interval);
  }, [speed]);

  return (
    <span className="inline-block animate-spin">
      {SPINNER_FRAMES[frame]}
    </span>
  );
}

// Fade in animation
interface FadeInProps {
  children: React.ReactNode;
  duration?: number;  // milliseconds
  delay?: number;
  className?: string;
}

export function FadeIn({
  children,
  duration = 300,
  delay = 0,
  className = '',
}: FadeInProps) {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const timer = setTimeout(() => {
      setVisible(true);
    }, delay);
    return () => clearTimeout(timer);
  }, [delay]);

  return (
    <div
      className={`${className} transition-opacity ${visible ? 'opacity-100' : 'opacity-0'}`}
      style={{ transitionDuration: `${duration}ms` }}
    >
      {children}
    </div>
  );
}

// Slide in animation
interface SlideInProps {
  children: React.ReactNode;
  direction?: 'up' | 'down' | 'left' | 'right';
  duration?: number;
  delay?: number;
  distance?: number;  // pixels
  className?: string;
}

export function SlideIn({
  children,
  direction = 'up',
  duration = 300,
  delay = 0,
  distance = 20,
  className = '',
}: SlideInProps) {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const timer = setTimeout(() => {
      setVisible(true);
    }, delay);
    return () => clearTimeout(timer);
  }, [delay]);

  const getTransform = () => {
    if (!visible) {
      switch (direction) {
        case 'up': return `translateY(${distance}px)`;
        case 'down': return `translateY(-${distance}px)`;
        case 'left': return `translateX(${distance}px)`;
        case 'right': return `translateX(-${distance}px)`;
      }
    }
    return 'translate(0)';
  };

  return (
    <div
      className={`${className} transition-all ${visible ? 'opacity-100' : 'opacity-0'}`}
      style={{
        transitionDuration: `${duration}ms`,
        transform: getTransform(),
      }}
    >
      {children}
    </div>
  );
}

// Scale animation
interface ScaleInProps {
  children: React.ReactNode;
  duration?: number;
  delay?: number;
  scale?: number;  // initial scale (e.g., 0.9)
  className?: string;
}

export function ScaleIn({
  children,
  duration = 300,
  delay = 0,
  scale = 0.9,
  className = '',
}: ScaleInProps) {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const timer = setTimeout(() => {
      setVisible(true);
    }, delay);
    return () => clearTimeout(timer);
  }, [delay]);

  return (
    <div
      className={`${className} transition-all ${visible ? 'opacity-100 scale-100' : 'opacity-0'}`}
      style={{
        transitionDuration: `${duration}ms`,
        transform: visible ? 'scale(1)' : `scale(${scale})`,
      }}
    >
      {children}
    </div>
  );
}

// Typewriter animation
interface TypewriterProps {
  text: string;
  speed?: number;  // milliseconds per character
  onComplete?: () => void;
  className?: string;
}

export function Typewriter({
  text,
  speed = 30,
  onComplete,
  className = '',
}: TypewriterProps) {
  const [displayedText, setDisplayedText] = useState('');

  useEffect(() => {
    setDisplayedText('');
    let index = 0;

    const interval = setInterval(() => {
      if (index < text.length) {
        setDisplayedText(text.slice(0, index + 1));
        index++;
      } else {
        clearInterval(interval);
        onComplete?.();
      }
    }, speed);

    return () => clearInterval(interval);
  }, [text, speed, onComplete]);

  return (
    <span className={className}>
      {displayedText}
      {/* Cursor */}
      <span className="inline-block w-0.5 h-4 ml-1 bg-current animate-pulse" />
    </span>
  );
}

// Pulse animation
interface PulseProps {
  children: React.ReactNode;
  duration?: number;
  className?: string;
}

export function Pulse({
  children,
  duration = 1000,
  className = '',
}: PulseProps) {
  return (
    <div
      className={`${className} animate-pulse`}
      style={{ animationDuration: `${duration}ms` }}
    >
      {children}
    </div>
  );
}

// Shake animation (for errors)
interface ShakeProps {
  children: React.ReactNode;
  active?: boolean;
  duration?: number;
  distance?: number;
  className?: string;
}

export function Shake({
  children,
  active = true,
  duration = 500,
  distance = 10,
  className = '',
}: ShakeProps) {
  const [shaking, setShaking] = useState(false);

  useEffect(() => {
    if (active) {
      setShaking(true);
      const timer = setTimeout(() => {
        setShaking(false);
      }, duration);
      return () => clearTimeout(timer);
    }
  }, [active, duration]);

  const keyframes = shaking ? {
    animation: `shake ${duration}ms ease-in-out`,
  } : {};

  return (
    <div className={className} style={keyframes}>
      {children}
    </div>
  );
}

// Bounce animation
interface BounceProps {
  children: React.ReactNode;
  delay?: number;
  className?: string;
}

export function Bounce({
  children,
  delay = 0,
  className = '',
}: BounceProps) {
  return (
    <div
      className={`${className} animate-bounce`}
      style={{ animationDelay: `${delay}ms` }}
    >
      {children}
    </div>
  );
}

// Message appear animation (staggered)
interface MessageAppearProps {
  children: React.ReactNode;
  index?: number;
  className?: string;
}

export function MessageAppear({
  children,
  index = 0,
  className = '',
}: MessageAppearProps) {
  const baseDelay = 50;
  const delay = baseDelay * Math.min(index, 10);

  return (
    <SlideIn
      direction="up"
      duration={300}
      delay={delay}
      className={className}
    >
      {children}
    </SlideIn>
  );
}

// Activity transition animation
interface ActivityTransitionProps {
  from: string;
  to: string;
  children: React.ReactNode;
  className?: string;
}

export function ActivityTransition({
  from,
  to,
  children,
  className = '',
}: ActivityTransitionProps) {
  const [isTransitioning, setIsTransitioning] = useState(false);

  useEffect(() => {
    if (from !== to) {
      setIsTransitioning(true);
      setTimeout(() => {
        setIsTransitioning(false);
      }, 200);
    }
  }, [from, to]);

  return (
    <div
      className={`${className} ${isTransitioning ? 'opacity-0 scale-95' : 'opacity-100 scale-100'} transition-all`}
      style={{ transitionDuration: '200ms' }}
    >
      {children}
    </div>
  );
}

// Expand/collapse animation
interface ExpandCollapseProps {
  children: React.ReactNode;
  expanded: boolean;
  duration?: number;
  maxHeight?: number;
  className?: string;
}

export function ExpandCollapse({
  children,
  expanded,
  duration = 200,
  maxHeight = 500,
  className = '',
}: ExpandCollapseProps) {
  return (
    <div
      className={`${className} overflow-hidden transition-all`}
      style={{
        maxHeight: expanded ? maxHeight : 0,
        opacity: expanded ? 1 : 0,
        transitionDuration: `${duration}ms`,
      }}
    >
      {children}
    </div>
  );
}

// Progress bar animation
interface AnimatedProgressProps {
  value: number;
  max: number;
  duration?: number;
  className?: string;
}

export function AnimatedProgress({
  value,
  max,
  duration = 500,
  className = '',
}: AnimatedProgressProps) {
  const percentage = Math.round((value / max) * 100);

  return (
    <div className={`${className} h-2 bg-muted rounded-full overflow-hidden`}>
      <div
        className="h-full bg-primary transition-all ease-out"
        style={{
          width: `${percentage}%`,
          transitionDuration: `${duration}ms`,
        }}
      />
    </div>
  );
}

// Notification pop animation
interface NotificationPopProps {
  children: React.ReactNode;
  className?: string;
}

export function NotificationPop({
  children,
  className = '',
}: NotificationPopProps) {
  return (
    <ScaleIn
      duration={200}
      scale={0.5}
      className={className}
    >
      {children}
    </ScaleIn>
  );
}

// Shimmer loading effect
interface ShimmerProps {
  width?: string;
  height?: string;
  className?: string;
}

export function Shimmer({
  width = '100%',
  height = '20px',
  className = '',
}: ShimmerProps) {
  return (
    <div
      className={`${className} relative overflow-hidden bg-muted`}
      style={{ width, height }}
    >
      <div
        className="absolute inset-0 bg-gradient-to-r from-transparent via-white/20 to-transparent"
        style={{
          animation: 'shimmer 2s infinite',
        }}
      />
    </div>
  );
}

// CSS animation keyframes (to be added to global CSS)
export const ANIMATION_KEYFRAMES = `
@keyframes shimmer {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(100%); }
}

@keyframes shake {
  0%, 100% { transform: translateX(0); }
  10%, 30%, 50%, 70%, 90% { transform: translateX(-10px); }
  20%, 40%, 60%, 80% { transform: translateX(10px); }
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

@keyframes slideInUp {
  from { transform: translateY(20px); opacity: 0; }
  to { transform: translateY(0); opacity: 1; }
}

@keyframes slideInDown {
  from { transform: translateY(-20px); opacity: 0; }
  to { transform: translateY(0); opacity: 1; }
}

@keyframes pulse-subtle {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.7; }
}
`;