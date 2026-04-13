const VOWELS = new Set(['a', 'e', 'i', 'o', 'u']);

function cleanWhitespace(value: string) {
  return value.trim().replace(/\s+/g, ' ');
}

function takeUpperSignal(word: string) {
  const letters = [...word].filter((char) => /[A-Za-z]/.test(char));
  if (letters.length === 0) {
    return '';
  }

  const signal = letters.filter((char, index) => index === 0 || char === char.toUpperCase()).join('');
  return signal.length >= 2 ? signal : '';
}

function takeDigits(word: string) {
  return [...word].filter((char) => /\d/.test(char)).join('');
}

function compactSingleWord(word: string, maxChars: number) {
  const digits = takeDigits(word);
  const letters = [...word].filter((char) => /[A-Za-z]/.test(char));
  if (letters.length === 0) {
    return digits.slice(0, maxChars);
  }

  if (digits) {
    return `${letters[0]}${digits}`.slice(0, maxChars);
  }

  if (word.length <= 8 && /[sS]$/.test(word)) {
    const vowel = letters.find((char, index) => index > 0 && VOWELS.has(char.toLowerCase()));
    const compactPlural = `${letters[0]}${vowel ?? ''}${letters[letters.length - 1]}`;
    if (compactPlural.length >= 3) {
      return compactPlural.slice(0, maxChars);
    }
  }

  const consonants = [letters[0], ...letters.slice(1).filter((char) => !VOWELS.has(char.toLowerCase()))];
  const uniqueChars: string[] = [];
  for (const char of consonants) {
    if (!uniqueChars.includes(char)) {
      uniqueChars.push(char);
    }
  }

  if (uniqueChars.length < Math.min(3, maxChars)) {
    for (const char of letters.slice(1)) {
      if (!uniqueChars.includes(char)) {
        uniqueChars.push(char);
      }
      if (uniqueChars.length >= maxChars) {
        break;
      }
    }
  }

  return uniqueChars.join('').slice(0, maxChars);
}

function abbreviateSegment(segment: string, maxChars: number): string {
  const words = cleanWhitespace(segment)
    .split(' ')
    .filter(Boolean);

  if (words.length === 0 || maxChars <= 0) {
    return '';
  }

  const tokenSignals = words.map((word) => {
    if (/^\d+$/.test(word)) {
      return word;
    }

    const upperSignal = takeUpperSignal(word);
    if (upperSignal) {
      return upperSignal;
    }

    return word[0] ?? '';
  });

  const combinedSignal = tokenSignals.join('');
  if (words.length > 1 && combinedSignal.length <= maxChars) {
    return combinedSignal;
  }

  if (words.length > 1) {
    const initials = tokenSignals.map((signal) => signal[0] ?? '').join('');
    if (initials.length <= maxChars) {
      return initials;
    }

    if (words.length >= 2 && maxChars >= 2) {
      return `${tokenSignals[0][0] ?? ''}${tokenSignals[tokenSignals.length - 1][0] ?? ''}`.slice(0, maxChars);
    }

    return initials.slice(0, maxChars);
  }

  const upperSignal = takeUpperSignal(words[0]);
  if (upperSignal) {
    return upperSignal.slice(0, maxChars);
  }

  return compactSingleWord(words[0], maxChars);
}

export function abbreviateTeamName(name: string, maxChars = 5) {
  const cleaned = cleanWhitespace(name);
  if (!cleaned) {
    return '';
  }

  if (cleaned.length <= maxChars) {
    return cleaned;
  }

  const segments = cleaned
    .split('/')
    .map((segment) => cleanWhitespace(segment))
    .filter(Boolean);

  if (segments.length === 0) {
    return cleaned.slice(0, maxChars);
  }

  if (segments.length === 1) {
    return abbreviateSegment(segments[0], maxChars);
  }

  const slashCount = segments.length - 1;
  const usableChars = Math.max(segments.length, maxChars - slashCount);
  const base = Math.floor(usableChars / segments.length);
  let remainder = usableChars % segments.length;

  const abbreviatedSegments = segments.map((segment) => {
    const budget = Math.max(1, base + (remainder > 0 ? 1 : 0));
    if (remainder > 0) {
      remainder -= 1;
    }
    return abbreviateSegment(segment, budget);
  });

  return abbreviatedSegments.join('/').slice(0, maxChars);
}

export function getTeamAbbreviation(name: string, customAbbreviation?: string | null, maxChars = 5) {
  const trimmedCustom = customAbbreviation?.trim();
  if (trimmedCustom) {
    return trimmedCustom.slice(0, maxChars);
  }

  return abbreviateTeamName(name, maxChars);
}