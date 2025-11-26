"use client";

import { Clock, Zap } from 'lucide-react';

interface PerformanceMetricsProps {
  ttft?: number | null; // Time to first token in microseconds
  totalTime?: number; // Total time in microseconds
  variant?: 'default' | 'compact' | 'inline';
  className?: string;
}

// Helper function to format time (microseconds to human-readable)
const formatTime = (time_us: number): string => {
  const time_ms = time_us / 1000;

  if (time_ms < 1000) {
    return `${time_ms.toFixed(2)}ms`;
  } else if (time_ms < 60000) {
    const seconds = (time_ms / 1000).toFixed(2);
    return `${seconds}s`;
  } else {
    const minutes = Math.floor(time_ms / 60000);
    const remainingMs = time_ms % 60000;
    const seconds = Math.floor(remainingMs / 1000);
    const ms = (remainingMs % 1000).toFixed(0);
    return `${minutes}:${seconds.toString().padStart(2, '0')}.${ms}`;
  }
};

export function PerformanceMetrics({
  ttft,
  totalTime,
  variant = 'default',
  className = ''
}: PerformanceMetricsProps) {
  // Don't render if no data
  if (totalTime === undefined && ttft === undefined) {
    return null;
  }

  // Compact variant for inline display (e.g., in chat messages)
  if (variant === 'compact') {
    return (
      <div className={`inline-flex items-center gap-1.5 ${className}`}>
        {ttft !== undefined && ttft !== null && (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 bg-emerald-50 text-emerald-700 rounded-md text-xs font-medium border border-emerald-200/50">
            <Zap className="h-3 w-3" />
            <span className="font-semibold">TTFT:</span>
            <span>{formatTime(ttft)}</span>
          </span>
        )}
      </div>
    );
  }

  // Inline variant for header areas
  if (variant === 'inline') {
    return (
      <div className={`inline-flex items-center gap-2 ${className}`}>
        {ttft !== undefined && ttft !== null && (
          <span className="inline-flex items-center gap-1.5 px-2.5 py-1 bg-emerald-50 text-emerald-700 rounded-lg text-xs font-semibold border border-emerald-200/60 shadow-sm">
            <Zap className="h-3.5 w-3.5" />
            <span>TTFT:</span>
            <span className="font-bold">{formatTime(ttft)}</span>
          </span>
        )}
        {totalTime !== undefined && (
          <span className="inline-flex items-center gap-1.5 px-2.5 py-1 bg-blue-50 text-blue-700 rounded-lg text-xs font-semibold border border-blue-200/60 shadow-sm">
            <Clock className="h-3.5 w-3.5" />
            <span>Total:</span>
            <span className="font-bold">{formatTime(totalTime)}</span>
          </span>
        )}
      </div>
    );
  }

  // Default variant - prominent display for summary panel
  return (
    <div className={`flex items-center gap-3 ${className}`}>
      {ttft !== undefined && ttft !== null && (
        <div className="flex items-center gap-2 px-3 py-2 bg-gradient-to-r from-emerald-50 to-emerald-100/80 text-emerald-700 rounded-lg border border-emerald-200/60 shadow-sm">
          <Zap className="h-4 w-4 text-emerald-600" />
          <div className="flex flex-col">
            <span className="text-[10px] font-medium text-emerald-600/80 uppercase tracking-wide">TTFT</span>
            <span className="text-sm font-bold leading-tight">{formatTime(ttft)}</span>
          </div>
        </div>
      )}
      {totalTime !== undefined && (
        <div className="flex items-center gap-2 px-3 py-2 bg-gradient-to-r from-blue-50 to-blue-100/80 text-blue-700 rounded-lg border border-blue-200/60 shadow-sm">
          <Clock className="h-4 w-4 text-blue-600" />
          <div className="flex flex-col">
            <span className="text-[10px] font-medium text-blue-600/80 uppercase tracking-wide">Total Time</span>
            <span className="text-sm font-bold leading-tight">{formatTime(totalTime)}</span>
          </div>
        </div>
      )}
    </div>
  );
}

