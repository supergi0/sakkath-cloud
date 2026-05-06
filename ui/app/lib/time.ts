const INDIA_TIME_ZONE = 'Asia/Kolkata';

function parseDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

export function formatIndiaTime(value: string) {
  const date = parseDate(value);
  if (!date) return '';

  return date.toLocaleTimeString('en-IN', {
    timeZone: INDIA_TIME_ZONE,
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  });
}

export function formatIndiaShortDate(value: string) {
  const date = parseDate(value);
  if (!date) return '';

  return date.toLocaleDateString('en-US', {
    timeZone: INDIA_TIME_ZONE,
    weekday: 'short',
    day: 'numeric',
    month: 'short',
  });
}

export function formatIndiaLongDate(value: string) {
  const date = parseDate(value);
  if (!date) return '';

  return date.toLocaleDateString('en-US', {
    timeZone: INDIA_TIME_ZONE,
    weekday: 'short',
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  });
}

export function formatIndiaShortDateTime(value: string) {
  const dateLabel = formatIndiaShortDate(value);
  const timeLabel = formatIndiaTime(value);

  if (!dateLabel) return timeLabel;
  if (!timeLabel) return dateLabel;
  return `${dateLabel} - ${timeLabel}`;
}