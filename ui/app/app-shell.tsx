'use client';

import { ThemeProvider } from './theme-provider';
import { AuthProvider } from './auth-provider';
import { Navbar } from './navbar';
import { Footer } from './footer';
import { SponsorBanner } from './sponsor-banner';

export function AppShell({ children }: { children: React.ReactNode }) {
  return (
    <ThemeProvider
      attribute="class"
      defaultTheme="dark"
      enableSystem={false}
      storageKey="sakkath-theme"
      themes={['light', 'dark']}
    >
      <AuthProvider>
        <Navbar />
        <main className="flex-1">{children}</main>
        <SponsorBanner />
        <Footer />
      </AuthProvider>
    </ThemeProvider>
  );
}