export function abbreviatePlayerName(name: string, maxLen = 20) {
  if (!name) return '';

  const trimmed = name.trim();
  if (trimmed.length <= maxLen) {
    return trimmed;
  }

  const parts = trimmed.split(/\s+/);
  if (parts.length <= 1) {
    return `${trimmed.slice(0, Math.max(0, maxLen - 3))}...`;
  }

  const current = [...parts];
  for (let index = current.length - 1; index > 0; index -= 1) {
    current[index] = `${current[index][0]}.`;
    const joined = current.join(' ');
    if (joined.length <= maxLen) {
      return joined;
    }
  }

  const shortened = current.join(' ');
  if (shortened.length <= maxLen) {
    return shortened;
  }

  return `${shortened.slice(0, Math.max(0, maxLen - 3))}...`;
}