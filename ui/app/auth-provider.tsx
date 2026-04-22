'use client';

import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { apiUrl } from './lib/api';

interface AuthState {
  isLoggedIn: boolean;
  role: number | null;
  roleName: string | null;
  token: string | null;
  isLoading: boolean;
}

interface AuthContextType extends AuthState {
  login: (token: string, role: number) => void;
  logout: () => void;
  verify: () => Promise<boolean>;
  isAdmin: boolean;
  isSuperAdmin: boolean;
  isPoc: boolean;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [authState, setAuthState] = useState<AuthState>({
    isLoggedIn: false,
    role: null,
    roleName: null,
    token: null,
    isLoading: true,
  });

  const login = useCallback((token: string, role: number) => {
    localStorage.setItem('auth_token', token);
    const roleName = role === 0 ? 'SUPER' : role === 1 ? 'ADMIN' : role === 3 ? 'MYTEAM' : null;
    setAuthState({
      isLoggedIn: true,
      role,
      roleName,
      token,
      isLoading: false,
    });
  }, []);

  const logout = useCallback(() => {
    localStorage.removeItem('auth_token');
    setAuthState({
      isLoggedIn: false,
      role: null,
      roleName: null,
      token: null,
      isLoading: false,
    });
  }, []);

  const verify = useCallback(async (): Promise<boolean> => {
    const token = localStorage.getItem('auth_token');
    if (!token) {
      setAuthState({ isLoggedIn: false, role: null, roleName: null, token: null, isLoading: false });
      return false;
    }

    try {
      const response = await fetch(apiUrl('/v1/auth/verify'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ token }),
      });

      if (!response.ok) {
        logout();
        return false;
      }

      const data = await response.json();
      if (data.valid && data.role !== null && data.role !== undefined) {
        const roleName = data.role === 0 ? 'SUPER' : data.role === 1 ? 'ADMIN' : data.role === 3 ? 'MYTEAM' : null;
        setAuthState({
          isLoggedIn: true,
          role: data.role,
          roleName,
          token,
          isLoading: false,
        });
        return true;
      } else {
        logout();
        return false;
      }
    } catch {
      logout();
      return false;
    }
  }, [logout]);

  // Verify on mount
  useEffect(() => {
    verify();
  }, [verify]);

  const isAdmin = authState.role === 0 || authState.role === 1;
  const isSuperAdmin = authState.role === 0;
  const isPoc = authState.role === 3;

  return (
    <AuthContext.Provider value={{ 
      ...authState, 
      login, 
      logout, 
      verify, 
      isAdmin, 
      isSuperAdmin,
      isPoc
    }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
}
