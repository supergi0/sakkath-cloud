'use client';

import Link from "next/link";
import { Text } from "./components/Text";

export function Footer() {
  return (
    <footer className="mt-auto bg-gray-100/95 dark:bg-slate-950/95 border-t border-gray-200 dark:border-slate-800 backdrop-blur">
      <div className="max-w-7xl mx-auto flex items-center justify-between px-4 py-4">
        <Link href="/" className="transition-colors hover:text-gray-700 dark:hover:text-gray-200">
          <Text variant="primary" className="text-lg font-semibold text-gray-900 dark:text-white">
            Sakkath Ultimate
          </Text>
        </Link>
        <div className="flex items-center gap-6">
          <a
            href="https://github.com/supergi0/sakkath-cloud"
            target="_blank"
            rel="noopener noreferrer"
            className="text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white transition-colors text-sm"
          >
            Contribute to Website
          </a>
          <a
            href="mailto:sakkathultimate@gmail.com"
            className="text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white transition-colors text-sm"
          >
            Contact Us
          </a>
        </div>
      </div>
    </footer>
  );
}
