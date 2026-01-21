'use client';

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Text } from "../components/Text";
import { Toast } from "../components/Toast";
import { useAuth } from "../auth-provider";

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:9000';

export default function Login() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [showToast, setShowToast] = useState(false);
  const [toastMessage, setToastMessage] = useState('');
  const router = useRouter();
  const { login } = useAuth();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);

    try {
      const response = await fetch(`${API_URL}/v1/auth/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, password }),
      });

      if (response.ok) {
        const data = await response.json();
        login(data.token, data.role);
        router.push('/');
      } else {
        setToastMessage('Invalid email or password');
        setShowToast(true);
      }
    } catch {
      setToastMessage('Login failed. Please try again.');
      setShowToast(true);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="py-8 px-4 md:px-0 min-h-screen flex items-center justify-center overflow-x-hidden">
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
            <Text variant="secondary" className="text-xs opacity-70">
              Test accounts: super@sakkath.com, admin1@sakkath.com, poc1@sakkath.com (pw: super@sakkath.com)
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

            <button
              type="submit"
              disabled={loading}
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
