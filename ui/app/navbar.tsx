'use client';

import { useState, useEffect } from 'react';
import { useTheme } from 'next-themes';
import { Sun, Moon, Menu, X} from 'lucide-react';
import Link from 'next/link';
import { usePathname, useRouter } from 'next/navigation';
import { Text } from './components/Text';
import { Toast } from './components/Toast';
import { useAuth } from './auth-provider';

const navItems = [
  { name: 'HOME', href: '/', clickable: true },
  { name: 'SCHEDULE', href: '/schedule', clickable: true },
  { name: 'PLAYER STATS', href: '/stats', clickable: true },
  { name: 'FIELDS', href: '/fields', clickable: true },
  { name: 'ANNOUNCEMENTS', href: '/announcements', clickable: true },
  { name: 'RULES', href: '/rules', clickable: true },
  { name: 'PHOTOS', href: '#', clickable: false },
];

// Mobile header items when logged in: HOME, ADMIN/SUPER, LOGOUT
const getMobileNavItems = (isLoggedIn: boolean, roleName: string | null) => {
  if (isLoggedIn) {
    const roleHref = roleName === 'MYTEAM' ? '/myteam' : '/admin';
    return [
      { name: 'HOME', href: '/', clickable: true },
      { name: roleName || 'ADMIN', href: roleHref, clickable: true, isRole: true },
      { name: 'LOGOUT', href: '#', clickable: true, isLogout: true },
    ];
  }
  return [
    { name: 'HOME', href: '/', clickable: true },
    { name: 'SCHEDULE', href: '/schedule', clickable: true },
    { name: 'STATS', href: '/stats', clickable: true },
    { name: 'UPDATES', href: '/announcements', clickable: true },
  ];
};

export function Navbar() {
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [showToast, setShowToast] = useState(false);
  const { resolvedTheme, setTheme } = useTheme();
  const pathname = usePathname();
  const router = useRouter();
  const { isLoggedIn, roleName, logout } = useAuth();

  useEffect(() => {
    if (sidebarOpen) {
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }
    return () => {
      document.body.style.overflow = '';
    };
  }, [sidebarOpen]);

  const toggleTheme = () => {
    const newTheme = resolvedTheme === 'dark' ? 'light' : 'dark';
    setTheme(newTheme);
  };

  const isActive = (href: string) => {
    if (href === '/') {
      return pathname === '/';
    }
    return pathname.startsWith(href);
  };

  const shellClasses = 'bg-gray-100/95 dark:bg-slate-950/95 border-b border-gray-200 dark:border-slate-800 backdrop-blur';
  const activeClasses = 'text-amber-700 dark:text-yellow-400';
  const idleClasses = 'text-gray-600 dark:text-slate-300 hover:bg-white/70 dark:hover:bg-slate-900/70 hover:text-gray-900 dark:hover:text-white';

  const handleItemClick = (item: { clickable: boolean; isLogout?: boolean; isRole?: boolean }) => {
    if (item.isLogout) {
      logout();
      router.push('/');
      return;
    }
    if (!item.clickable || item.isRole) {
      setShowToast(true);
    }
  };

  const handleLogout = () => {
    logout();
    router.push('/');
  };

  const mobileNavItems = getMobileNavItems(isLoggedIn, roleName);

  return (
    <>
      <Toast 
        message="Coming soon!" 
        isOpen={showToast} 
        onClose={() => setShowToast(false)} 
        duration={3000}
      />
      {/* Desktop Navbar */}
      <nav className={`hidden md:block sticky top-0 z-50 ${shellClasses}`}>
        <div className="max-w-7xl mx-auto flex items-center justify-between px-4 py-4">
          <div className="flex items-center gap-1">
            {navItems.map((item) => (
              item.clickable ? (
                <Link
                  key={item.name}
                  href={item.href}
                  className={`px-4 py-2 text-sm font-bold transition-colors rounded ${
                    isActive(item.href)
                      ? activeClasses
                      : idleClasses
                  }`}
                >
                  {item.name}
                </Link>
              ) : (
                <button
                  key={item.name}
                  onClick={() => handleItemClick(item)}
                  className={`px-4 py-2 text-sm font-bold transition-colors rounded ${idleClasses}`}
                >
                  {item.name}
                </button>
              )
            ))}
          </div>
          <div className="flex items-center gap-4">
            {isLoggedIn ? (
              <>
                {roleName === 'MYTEAM' ? (
                  <Link
                    href="/myteam"
                    className={`px-4 py-2 text-sm font-bold transition-colors rounded ${
                      pathname === '/myteam'
                        ? 'text-amber-700 dark:text-yellow-400'
                        : 'text-yellow-600 dark:text-yellow-400 hover:bg-white/70 dark:hover:bg-slate-900/70'
                    }`}
                  >
                    {roleName}
                  </Link>
                ) : (
                  <Link
                    href="/admin"
                    className={`px-4 py-2 text-sm font-bold transition-colors rounded ${
                      pathname === '/admin'
                        ? 'text-amber-700 dark:text-yellow-400'
                        : 'text-yellow-600 dark:text-yellow-400 hover:bg-white/70 dark:hover:bg-slate-900/70'
                    }`}
                  >
                    {roleName}
                  </Link>
                )}
                <button
                  onClick={handleLogout}
                  className={`px-4 py-2 text-sm font-bold transition-colors rounded ${idleClasses}`}
                >
                  LOGOUT
                </button>
              </>
            ) : (
              <Link
                href="/login"
                className={`px-4 py-2 text-sm font-bold transition-colors rounded ${idleClasses}`}
              >
                LOGIN
              </Link>
            )}
            <button
              onClick={toggleTheme}
              className="p-2 rounded hover:bg-white/70 dark:hover:bg-slate-900/70 transition-colors"
              aria-label="Toggle theme"
            >
              <Sun className="w-5 h-5 text-amber-500 block dark:hidden" />
              <Moon className="w-5 h-5 text-slate-300 hidden dark:block" />
            </button>
          </div>
        </div>
      </nav>

      {/* Mobile Navbar */}
      <nav className={`sticky top-0 z-50 flex h-14 items-center justify-between px-2 md:hidden ${shellClasses}`}>
        <button
          onClick={() => setSidebarOpen(true)}
          className="p-2 rounded hover:bg-white/70 dark:hover:bg-slate-900/70 transition-colors"
          aria-label="Open menu"
        >
          <Menu className="w-5 h-5 text-gray-800 dark:text-white" />
        </button>
        
        <div className="flex items-center gap-0.5">
          {mobileNavItems.map((item) => (
            item.clickable && !item.isLogout ? (
              <Link
                key={item.name}
                href={item.href}
                className={`px-2 py-1.5 text-xs font-bold transition-colors rounded ${
                  isActive(item.href)
                    ? activeClasses
                    : idleClasses
                }`}
              >
                {item.name}
              </Link>
            ) : item.isLogout ? (
              <button
                key={item.name}
                onClick={handleLogout}
                className={`px-2 py-1.5 text-xs font-bold transition-colors rounded ${idleClasses}`}
              >
                {item.name}
              </button>
            ) : item.isRole ? (
              roleName === 'MYTEAM' ? (
                <Link
                  key={item.name}
                  href="/myteam"
                  className={`px-2 py-1.5 text-xs font-bold transition-colors rounded ${
                    pathname === '/myteam'
                      ? 'text-amber-700 dark:text-yellow-400'
                      : 'text-yellow-600 dark:text-yellow-400 hover:bg-white/70 dark:hover:bg-slate-900/70'
                  }`}
                >
                  {item.name}
                </Link>
              ) : (
                <Link
                  key={item.name}
                  href="/admin"
                  className={`px-2 py-1.5 text-xs font-bold transition-colors rounded ${
                    pathname === '/admin'
                      ? 'text-amber-700 dark:text-yellow-400'
                      : 'text-yellow-600 dark:text-yellow-400 hover:bg-white/70 dark:hover:bg-slate-900/70'
                  }`}
                >
                  {item.name}
                </Link>
              )
            ) : (
              <button
                key={item.name}
                onClick={() => handleItemClick(item)}
                className={`px-2 py-1.5 text-xs font-bold transition-colors rounded ${idleClasses}`}
              >
                {item.name}
              </button>
            )
          ))}
        </div>

        <button
          onClick={toggleTheme}
          className="p-2 rounded hover:bg-white/70 dark:hover:bg-slate-900/70 transition-colors"
          aria-label="Toggle theme"
        >
          <Sun className="w-5 h-5 text-amber-500 block dark:hidden" />
          <Moon className="w-5 h-5 text-slate-300 hidden dark:block" />
        </button>
      </nav>

      {/* Mobile Sidebar Overlay - covers everything including navbar */}
      {sidebarOpen && (
        <div
          className="fixed inset-0 bg-black/50 z-[55] md:hidden"
          onClick={() => setSidebarOpen(false)}
        />
      )}

      {/* Mobile Sidebar - All nav items except ADMIN role */}
      <div
        className={`fixed top-0 left-0 h-full w-48 bg-gray-100 dark:bg-slate-950 border-r border-gray-200 dark:border-slate-800 z-[60] transform transition-transform duration-300 ease-in-out md:hidden ${
          sidebarOpen ? 'translate-x-0' : '-translate-x-full'
        }`}
      >
        <div className="flex items-center justify-end p-3 border-b border-gray-200 dark:border-slate-800">
          <button
            onClick={() => setSidebarOpen(false)}
            className="p-1.5 rounded hover:bg-white/70 dark:hover:bg-slate-900/70 transition-colors"
            aria-label="Close menu"
          >
            <X className="w-5 h-5 text-gray-800 dark:text-white" />
          </button>
        </div>
        <div className="py-2">
          {navItems.map((item) => (
            item.clickable ? (
              <Link
                key={item.name}
                href={item.href}
                onClick={() => setSidebarOpen(false)}
                className={`block px-4 py-2.5 text-sm font-bold transition-all ${
                  isActive(item.href)
                    ? 'text-amber-700 dark:text-yellow-400'
                    : 'text-gray-600 dark:text-slate-300 hover:bg-white/70 dark:hover:bg-slate-900/70 hover:text-gray-900 dark:hover:text-white'
                }`}
              >
                {item.name}
              </Link>
            ) : (
              <button
                key={item.name}
                onClick={() => {
                  setSidebarOpen(false);
                  handleItemClick(item);
                }}
                className="block w-full text-left px-4 py-2.5 text-sm font-bold transition-all text-gray-600 dark:text-slate-300 hover:bg-white/70 dark:hover:bg-slate-900/70 hover:text-gray-900 dark:hover:text-white"
              >
                {item.name}
              </button>
            )
          ))}
          {isLoggedIn && roleName === 'MYTEAM' && (
            <Link
              href="/myteam"
              onClick={() => setSidebarOpen(false)}
              className={`block px-4 py-2.5 text-sm font-bold transition-all border-t border-gray-200 dark:border-slate-800 ${
                pathname === '/myteam'
                  ? 'text-amber-700 dark:text-yellow-400'
                  : 'text-yellow-600 dark:text-yellow-400 hover:bg-white/70 dark:hover:bg-slate-900/70'
              }`}
            >
              MY TEAM
            </Link>
          )}
          {isLoggedIn && (roleName === 'ADMIN' || roleName === 'SUPER') && (
            <Link
              href="/admin"
              onClick={() => setSidebarOpen(false)}
              className={`block px-4 py-2.5 text-sm font-bold transition-all border-t border-gray-200 dark:border-slate-800 ${
                pathname === '/admin'
                  ? 'text-amber-700 dark:text-yellow-400'
                  : 'text-yellow-600 dark:text-yellow-400 hover:bg-white/70 dark:hover:bg-slate-900/70'
              }`}
            >
              ADMIN
            </Link>
          )}
          {!isLoggedIn && (
            <Link
              href="/login"
              onClick={() => setSidebarOpen(false)}
              className={`block px-4 py-2.5 text-sm font-bold transition-all border-t border-gray-200 dark:border-slate-800 ${
                pathname === '/login'
                  ? 'text-amber-700 dark:text-yellow-400'
                  : 'text-gray-600 dark:text-slate-300 hover:bg-white/70 dark:hover:bg-slate-900/70 hover:text-gray-900 dark:hover:text-white'
              }`}
            >
              LOGIN
            </Link>
          )}
          {isLoggedIn && (
            <button
              onClick={() => {
                setSidebarOpen(false);
                handleLogout();
              }}
              className="block w-full text-left px-4 py-2.5 text-sm font-bold transition-all text-gray-600 dark:text-slate-300 hover:bg-white/70 dark:hover:bg-slate-900/70 hover:text-gray-900 dark:hover:text-white border-t border-gray-200 dark:border-slate-800 mt-2"
            >
              LOGOUT
            </button>
          )}
        </div>
      </div>
    </>
  );
}
