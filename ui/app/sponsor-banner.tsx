'use client';

import { Text } from "./components/Text";

export function SponsorBanner() {
  const sponsors = [
    { id: 1, name: "Sponsor 1" },
    { id: 2, name: "Sponsor 2" },
    { id: 3, name: "Sponsor 3" },
    { id: 4, name: "Sponsor 4" },
  ];

  return (
    <div className="bg-white dark:bg-slate-900 py-8 mt-8">
      <div className="max-w-7xl mx-auto px-4">
        <div className="grid grid-cols-4 gap-4">
          {sponsors.map((sponsor) => (
            <div
              key={sponsor.id}
              className="flex items-center justify-center aspect-square max-w-[120px] mx-auto p-4 bg-gray-100 dark:bg-slate-800 rounded-lg hover:opacity-80 transition-opacity"
            >
              <Text variant="secondary" className="text-xs font-medium text-center">
                {sponsor.name}
              </Text>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
