'use client';

import { useEffect, useMemo, useState } from 'react';

const EVENT_START = '2026-05-22T00:00:00+05:30';
const TYPE_SPEED_MS = 55;
const DELETE_SPEED_MS = 28;
const HOLD_MS = 5000;

function getCountdownValues(now: Date) {
  const targetTime = new Date(EVENT_START).getTime();
  const diffMs = Math.max(0, targetTime - now.getTime());
  const daysAway = Math.max(0, Math.ceil(diffMs / (1000 * 60 * 60 * 24)));
  const weekendsAway = Math.max(0, Math.ceil(daysAway / 7));

  return {
    weekends: `${weekendsAway} ${weekendsAway === 1 ? 'weekend' : 'weekends'}`,
    days: `${daysAway} ${daysAway === 1 ? 'day' : 'days'}`,
  };
}

export function LandingCountdown() {
  const [now, setNow] = useState(() => new Date());
  const [messageIndex, setMessageIndex] = useState(0);
  const [displayText, setDisplayText] = useState('');
  const [isDeleting, setIsDeleting] = useState(false);

  useEffect(() => {
    const clockTimer = window.setInterval(() => setNow(new Date()), 60_000);

    return () => {
      window.clearInterval(clockTimer);
    };
  }, []);

  const countdown = useMemo(() => getCountdownValues(now), [now]);
  const messages = useMemo(() => [countdown.weekends, countdown.days], [countdown.days, countdown.weekends]);

  useEffect(() => {
    const activeMessage = messages[messageIndex] ?? '';

    if (!isDeleting && displayText === activeMessage) {
      const holdTimer = window.setTimeout(() => {
        setIsDeleting(true);
      }, HOLD_MS);

      return () => window.clearTimeout(holdTimer);
    }

    const delay = isDeleting ? DELETE_SPEED_MS : TYPE_SPEED_MS;
    const timer = window.setTimeout(() => {
      if (isDeleting) {
        const nextText = activeMessage.slice(0, Math.max(0, displayText.length - 1));
        setDisplayText(nextText);

        if (nextText.length === 0) {
          setIsDeleting(false);
          setMessageIndex((current) => (current + 1) % messages.length);
        }

        return;
      }

      setDisplayText(activeMessage.slice(0, displayText.length + 1));
    }, delay);

    return () => window.clearTimeout(timer);
  }, [displayText, isDeleting, messageIndex, messages]);

  return (
    <p className="text-balance text-base font-medium tracking-[-0.02em] text-gray-900 dark:text-white md:text-lg">
      <span>Coming to Bangalore in </span>
      {displayText}
      <span className="countdown-cursor ml-1 inline-block h-[1.05em] w-[3px] align-[-0.12em]" />
    </p>
  );
}