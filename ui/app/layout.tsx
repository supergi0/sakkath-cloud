import type { Metadata } from "next";
import { Inter } from "next/font/google";
import "./globals.css";
import { ThemeProvider } from "./theme-provider";
import { AuthProvider } from "./auth-provider";
import { Navbar } from "./navbar";
import { Footer } from "./footer";
import { SponsorBanner } from "./sponsor-banner";

const inter = Inter({
  subsets: ["latin"],
  variable: "--font-inter",
});

export const metadata: Metadata = {
  title: "Sakkath 2026",
  description: "Sakkath Ultimate Open 2026",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className={`${inter.variable} antialiased bg-gray-100/70 dark:bg-slate-950 min-h-screen flex flex-col`}>
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
      </body>
    </html>
  );
}
