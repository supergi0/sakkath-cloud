'use client';

import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { apiUrl } from './lib/api';

function readStoredAuthToken() {
  if (typeof window === 'undefined') {
    return null;
  }

  return localStorage.getItem('auth_token');
}

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
  isAdmin: boolean;
  isSuperAdmin: boolean;
  isPoc: boolean;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [authState, setAuthState] = useState<AuthState>(() => {
    const token = readStoredAuthToken();
    return {
      isLoggedIn: false,
      role: null,
      roleName: null,
      token,
      isLoading: Boolean(token),
    };
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

  // Verify on mount
  useEffect(() => {
    if (!authState.token) {
      return;
    }

    let cancelled = false;

    const verifyStoredToken = async () => {
      try {
        const response = await fetch(apiUrl('/v1/auth/verify'), {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ token: authState.token }),
        });

        if (!response.ok) {
          if (!cancelled) {
            logout();
          }
          return;
        }

        const data = await response.json();
        if (data.valid && data.role !== null && data.role !== undefined) {
          const roleName = data.role === 0 ? 'SUPER' : data.role === 1 ? 'ADMIN' : data.role === 3 ? 'MYTEAM' : null;
          if (!cancelled) {
            setAuthState({
              isLoggedIn: true,
              role: data.role,
              roleName,
              token: authState.token,
              isLoading: false,
            });
          }
          return;
        }

        if (!cancelled) {
          logout();
        }
      } catch {
        if (!cancelled) {
          logout();
        }
      }
    };

    void verifyStoredToken();

    return () => {
      cancelled = true;
    };
  }, [authState.token, logout]);

  const isAdmin = authState.role === 0 || authState.role === 1;
  const isSuperAdmin = authState.role === 0;
  const isPoc = authState.role === 3;

  return (
    <AuthContext.Provider value={{ 
      ...authState, 
      login, 
      logout, 
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
