'use client';

import { ReactNode } from 'react';

interface TextProps {
  children: ReactNode;
  variant?: 'primary' | 'secondary';
  className?: string;
  as?: 'span' | 'p' | 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6' | 'div';
  title?: string;
}

export function Text({ 
  children, 
  variant = 'primary', 
  className = '',
  as: Component = 'span',
  title,
}: TextProps) {
  const variantClasses = {
    primary: 'text-gray-900 dark:text-white',
    secondary: 'text-gray-600 dark:text-gray-400',
  };

  return (
    <Component className={`${variantClasses[variant]} ${className}`} title={title}>
      {children}
    </Component>
  );
}
