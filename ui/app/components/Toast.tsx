'use client';

import { useEffect } from 'react';
import { useTheme } from 'next-themes';
import { X } from 'lucide-react';

interface ToastProps {
  message: string;
  isOpen: boolean;
  onClose: () => void;
  duration?: number;
}

export function Toast({ message, isOpen, onClose, duration = 3000 }: ToastProps) {
  const { theme } = useTheme();
  
  useEffect(() => {
    if (isOpen && duration > 0) {
      const timer = setTimeout(() => {
        onClose();
      }, duration);
      return () => clearTimeout(timer);
    }
  }, [isOpen, duration, onClose]);

  if (!isOpen) return null;

  const isDark = theme === 'dark';

  return (
    <div className="fixed bottom-6 left-6 z-50 animate-in slide-in-from-bottom-5 fade-in">
      <div className={`border rounded-lg shadow-lg p-4 pr-12 max-w-md ${
        isDark 
          ? 'bg-white border-gray-200' 
          : 'bg-slate-800 border-slate-700'
      }`}>
        <p className={`text-sm font-medium ${isDark ? 'text-gray-900' : 'text-white'}`}>
          {message}
        </p>
        <button
          onClick={onClose}
          className={`absolute top-3 right-3 p-1 rounded transition-colors ${
            isDark
              ? 'hover:bg-gray-100 text-gray-500'
              : 'hover:bg-slate-700 text-gray-400'
          }`}
          aria-label="Close toast"
        >
          <X className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}
