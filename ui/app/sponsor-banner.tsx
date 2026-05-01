'use client';

import Image from 'next/image';

export function SponsorBanner() {
  const sponsors = [
    {
      id: 1,
      name: 'Peak Performance',
      logo: '/peakpf.png',
      layoutClassName: 'sm:col-span-2 sm:col-start-2',
      containerClassName: 'w-[240px] sm:w-[320px]',
      imageHeightClassName: 'h-[56px] sm:h-[72px]',
    },
  ];

  if (sponsors.length === 0) {
    return null;
  }

  return (
    <div className="mt-3 bg-white py-6 dark:bg-slate-900 sm:mt-6 sm:py-8">
      <div className="max-w-7xl mx-auto px-4">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-4">
          {sponsors.map((sponsor) => (
            <div
              key={sponsor.id}
              className={`mx-auto flex items-center justify-center ${sponsor.layoutClassName}`}
            >
              <div className={`relative ${sponsor.containerClassName} ${sponsor.imageHeightClassName}`}>
                <Image
                  src={sponsor.logo}
                  alt={sponsor.name}
                  fill
                  className="object-contain"
                  sizes="(max-width: 639px) 240px, 320px"
                />
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
