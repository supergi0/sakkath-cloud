'use client';

import Script from "next/script";
import { useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { Text } from "../components/Text";
import { Toast } from "../components/Toast";
import { useAuth } from "../auth-provider";
import { apiUrl } from "../lib/api";
const TURNSTILE_SITE_KEY = process.env.NEXT_PUBLIC_TURNSTILE_SITE_KEY || '';

declare global {
  interface Window {
    turnstile?: {
      render: (
        container: string | HTMLElement,
        options: {
          sitekey: string;
          theme?: 'light' | 'dark' | 'auto';
          callback?: (token: string) => void;
          'expired-callback'?: () => void;
          'error-callback'?: () => void;
        }
      ) => string;
      reset: (widgetId?: string) => void;
    };
  }
}

export default function Login() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [captchaToken, setCaptchaToken] = useState('');
  const [loading, setLoading] = useState(false);
  const [showToast, setShowToast] = useState(false);
  const [toastMessage, setToastMessage] = useState('');
  const widgetIdRef = useRef<string | null>(null);
  const router = useRouter();
  const { login } = useAuth();

  const resetCaptcha = () => {
    setCaptchaToken('');

    if (widgetIdRef.current && window.turnstile) {
      window.turnstile.reset(widgetIdRef.current);
    }
  };

  const renderCaptcha = () => {
    if (!TURNSTILE_SITE_KEY || !window.turnstile || widgetIdRef.current) {
      return;
    }

    widgetIdRef.current = window.turnstile.render('#turnstile-widget', {
      sitekey: TURNSTILE_SITE_KEY,
      theme: 'auto',
      callback: (token: string) => setCaptchaToken(token),
      'expired-callback': () => setCaptchaToken(''),
      'error-callback': () => {
        setCaptchaToken('');
        setToastMessage('Captcha failed to load. Please retry.');
        setShowToast(true);
      },
    });
  };

  useEffect(() => {
    renderCaptcha();
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (TURNSTILE_SITE_KEY && !captchaToken) {
      setToastMessage('Please complete the captcha.');
      setShowToast(true);
      return;
    }

    setLoading(true);

    try {
      const response = await fetch(apiUrl('/v1/auth/login'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, password, captcha_token: captchaToken || undefined }),
      });

      if (response.ok) {
        const data = await response.json();
        login(data.token, data.role);
        router.push('/');
      } else if (response.status === 403) {
        setToastMessage('Captcha verification failed. Please try again.');
        setShowToast(true);
        resetCaptcha();
      } else {
        setToastMessage('Invalid email or password');
        setShowToast(true);
        resetCaptcha();
      }
    } catch {
      setToastMessage('Login failed. Please try again.');
      setShowToast(true);
      resetCaptcha();
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="py-8 px-4 md:px-0 min-h-screen flex items-center justify-center overflow-x-hidden">
      {TURNSTILE_SITE_KEY ? (
        <Script
          src="https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit"
          strategy="afterInteractive"
          onLoad={renderCaptcha}
        />
      ) : null}

      <Toast 
        message={toastMessage} 
        isOpen={showToast} 
        onClose={() => setShowToast(false)} 
        duration={3000}
      />
      
      <div className="max-w-md w-full">
        <div className="rounded-sm p-8 bg-white dark:bg-slate-900">
          <Text as="h1" variant="primary" className="text-2xl font-semibold mb-6 text-center">
            Login
          </Text>
          
          <div className="mb-6 p-4 rounded bg-gray-100 dark:bg-slate-800">
            <Text variant="secondary" className="text-sm mb-2">
              <strong>For Team Admins and Volunteers use only. </strong>
            </Text>
            <Text variant="secondary" className="text-sm mb-3">
                If you want to edit info, scores, spirit scores, please contact your designated volunteer.
            </Text>
          </div>

          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label htmlFor="email" className="block mb-2">
                <Text variant="primary" className="text-sm font-medium">
                  Email
                </Text>
              </label>
              <input
                id="email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                className="w-full px-4 py-2 rounded border border-gray-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-900"
                placeholder="Enter your email"
                required
              />
            </div>

            <div>
              <label htmlFor="password" className="block mb-2">
                <Text variant="primary" className="text-sm font-medium">
                  Password
                </Text>
              </label>
              <input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="w-full px-4 py-2 rounded border border-gray-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-900"
                placeholder="Enter your password"
                required
              />
            </div>

            {TURNSTILE_SITE_KEY ? (
              <div className="rounded border border-gray-200 bg-gray-50 p-3 dark:border-slate-700 dark:bg-slate-800/80">
                <div id="turnstile-widget" className="min-h-[65px]" />
                <Text variant="secondary" className="mt-2 text-xs">
                  Confirm you are not a bot before signing in.
                </Text>
              </div>
            ) : null}

            <button
              type="submit"
              disabled={loading || (Boolean(TURNSTILE_SITE_KEY) && !captchaToken)}
              className="w-full px-4 py-2.5 rounded bg-blue-900 hover:bg-blue-950 text-white font-medium transition-colors disabled:opacity-50"
            >
              {loading ? 'Logging in...' : 'Login'}
            </button>
          </form>
        </div>
      </div>
    </div>
  );
}
