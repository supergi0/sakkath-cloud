'use client';

import { Text } from "./components/Text";

export function Footer() {
  return (
    <footer className="bg-blue-950 mt-auto">
      <div className="max-w-7xl mx-auto flex items-center justify-between px-4 py-4">
        <Text variant="primary" className="text-white text-lg font-semibold">
          Sakkath Ultimate
        </Text>
        <div className="flex items-center gap-6">
          <a
            href="https://github.com/supergi0"
            target="_blank"
            rel="noopener noreferrer"
            className="text-gray-300 hover:text-white transition-colors text-sm"
          >
            Contribute to Website
          </a>
          <a
            href="mailto:sakkath@gmail.com"
            className="text-gray-300 hover:text-white transition-colors text-sm"
          >
            Contact Us
          </a>
        </div>
      </div>
    </footer>
  );
}
