'use client';

import { ReactNode, useEffect, useState } from "react";
import { Text } from "../components/Text";

type HandbookSectionKey = 'rules' | 'format' | 'reporting' | 'edit-team';

const HANDBOOK_SECTIONS: { key: HandbookSectionKey; label: string; file: string }[] = [
  { key: 'rules', label: 'Rules', file: '/rules.md' },
  { key: 'format', label: 'Format', file: '/format.md' },
  { key: 'reporting', label: 'Reporting', file: '/reporting.md' },
  { key: 'edit-team', label: 'Edit Team', file: '/edit-team.md' },
];

const INLINE_MARKDOWN_PATTERN = /(\*\*[^*]+\*\*|`[^`]+`|\*[^*]+\*)/;

type HandbookContents = Record<HandbookSectionKey, string>;

function renderInlineMarkdown(text: string, keyPrefix: string): ReactNode[] {
  const elements: ReactNode[] = [];
  let remaining = text;
  let index = 0;

  while (remaining.length > 0) {
    const match = INLINE_MARKDOWN_PATTERN.exec(remaining);

    if (!match || match.index === undefined) {
      elements.push(<span key={`${keyPrefix}-${index++}`}>{remaining}</span>);
      break;
    }

    if (match.index > 0) {
      elements.push(
        <span key={`${keyPrefix}-${index++}`}>
          {remaining.slice(0, match.index)}
        </span>
      );
    }

    const token = match[0];
    const content = token.slice(token.startsWith('**') ? 2 : 1, token.endsWith('**') ? -2 : -1);

    if (token.startsWith('**') && token.endsWith('**')) {
      elements.push(
        <strong key={`${keyPrefix}-${index++}`} className="font-bold">
          {content}
        </strong>
      );
    } else if (token.startsWith('`') && token.endsWith('`')) {
      elements.push(
        <code
          key={`${keyPrefix}-${index++}`}
          className="rounded bg-gray-100 px-1 py-0.5 font-mono text-[0.95em] text-blue-900 dark:bg-slate-800 dark:text-cyan-200"
        >
          {content}
        </code>
      );
    } else {
      elements.push(
        <em key={`${keyPrefix}-${index++}`} className="italic">
          {content}
        </em>
      );
    }

    remaining = remaining.slice(match.index + token.length);
  }

  return elements;
}

function isItalicOnlyLine(line: string) {
  const trimmed = line.trim();
  return trimmed.startsWith('*') && trimmed.endsWith('*') && !trimmed.startsWith('**') && trimmed.length > 2;
}

function listIndentStyle(line: string) {
  const indent = line.match(/^\s*/)?.[0].length ?? 0;
  return { marginLeft: `${1.5 + Math.floor(indent / 2)}rem` };
}

async function loadHandbookContents(): Promise<HandbookContents> {
  const cacheBust = Date.now();

  const entries = await Promise.all(
    HANDBOOK_SECTIONS.map(async (section) => {
      const response = await fetch(`${section.file}?v=${cacheBust}`, {
        cache: 'no-store',
        headers: { 'cache-control': 'no-cache' },
      });

      if (!response.ok) {
        throw new Error(`Failed to load ${section.file}`);
      }

      const text = await response.text();
      return [section.key, text] as const;
    })
  );

  return {
    rules: entries.find(([key]) => key === 'rules')?.[1] ?? '',
    format: entries.find(([key]) => key === 'format')?.[1] ?? '',
    reporting: entries.find(([key]) => key === 'reporting')?.[1] ?? '',
    'edit-team': entries.find(([key]) => key === 'edit-team')?.[1] ?? '',
  };
}

function renderMarkdown(content: string) {
  const lines = content.split('\n');
  const elements: ReactNode[] = [];

  lines.forEach((line, idx) => {
    const trimmed = line.trim();

    if (line.startsWith('# ')) {
      elements.push(
        <Text key={idx} as="h1" variant="primary" className="text-3xl font-bold mt-6 mb-4">
          {renderInlineMarkdown(line.substring(2), `h1-${idx}`)}
        </Text>
      );
    } else if (line.startsWith('## ')) {
      elements.push(
        <Text key={idx} as="h2" variant="primary" className="text-2xl font-semibold mt-6 mb-3">
          {renderInlineMarkdown(line.substring(3), `h2-${idx}`)}
        </Text>
      );
    } else if (line.startsWith('### ')) {
      elements.push(
        <Text key={idx} as="h3" variant="primary" className="text-xl font-medium mt-4 mb-2">
          {renderInlineMarkdown(line.substring(4), `h3-${idx}`)}
        </Text>
      );
    } else if (/^\s*-\s+/.test(line)) {
      const text = line.replace(/^\s*-\s+/, '');

      elements.push(
        <li key={idx} className="mb-2" style={listIndentStyle(line)}>
          <Text variant="primary" className="inline">
            {renderInlineMarkdown(text, `li-${idx}`)}
          </Text>
        </li>
      );
    } else if (/^\s*\d+\.\s/.test(line)) {
      elements.push(
        <div key={idx} className="mb-2" style={listIndentStyle(line)}>
          <Text as="span" variant="primary">
            {renderInlineMarkdown(trimmed, `ol-${idx}`)}
          </Text>
        </div>
      );
    } else if (isItalicOnlyLine(line)) {
      elements.push(
        <Text key={idx} as="p" variant="secondary" className="italic text-sm mt-4">
          {renderInlineMarkdown(trimmed.substring(1, trimmed.length - 1), `em-${idx}`)}
        </Text>
      );
    } else if (trimmed === '---') {
      elements.push(<hr key={idx} className="my-6 border-gray-300 dark:border-slate-700" />);
    } else if (trimmed !== '') {
      elements.push(
        <Text key={idx} as="p" variant="primary" className="mb-3">
          {renderInlineMarkdown(line, `p-${idx}`)}
        </Text>
      );
    } else {
      elements.push(<div key={idx} className="h-2" />);
    }
  });

  return elements;
}

export default function Handbook() {
  const [activeSection, setActiveSection] = useState<HandbookSectionKey>('rules');
  const [contents, setContents] = useState<HandbookContents>({
    rules: '',
    format: '',
    reporting: '',
    'edit-team': '',
  });
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    const loadContents = async () => {
      try {
        const nextContents = await loadHandbookContents();

        if (cancelled) {
          return;
        }

        setContents(nextContents);
        setLoading(false);
      } catch {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    const handleWindowFocus = () => {
      void loadContents();
    };

    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible') {
        void loadContents();
      }
    };

    void loadContents();
    window.addEventListener('focus', handleWindowFocus);
    document.addEventListener('visibilitychange', handleVisibilityChange);

    return () => {
      cancelled = true;
      window.removeEventListener('focus', handleWindowFocus);
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, []);

  if (loading) {
    return (
      <div className="py-4 px-4 md:px-0 min-h-screen bg-gray-100 dark:bg-slate-950">
        <div className="max-w-7xl mx-auto">
          <Text variant="primary">Loading...</Text>
        </div>
      </div>
    );
  }

  const activeContent = contents[activeSection].trim();

  return (
    <div className="py-6 px-4 md:px-0 min-h-screen bg-gray-100 dark:bg-slate-950">
      <div className="max-w-4xl mx-auto">
        <div className="rounded-sm p-6 md:p-8 bg-white dark:bg-slate-900">
          <div className="mb-6 flex flex-col gap-4">
            <div className="flex items-center gap-3">
              <div>
                <Text as="h1" variant="primary" className="text-2xl font-semibold">
                  Handbook
                </Text>
              </div>
            </div>
            <div className="hidden gap-2 sm:flex">
              {HANDBOOK_SECTIONS.map((section) => (
                <button
                  key={section.key}
                  onClick={() => setActiveSection(section.key)}
                  className={`rounded px-4 py-2 text-sm font-medium transition-colors ${
                    activeSection === section.key
                      ? 'bg-blue-900 text-white'
                      : 'bg-gray-200 text-gray-700 hover:bg-gray-300 dark:bg-slate-800 dark:text-gray-300 dark:hover:bg-slate-700'
                  }`}
                >
                  {section.label}
                </button>
              ))}
            </div>
            <div className="grid grid-cols-2 gap-2 sm:hidden">
              {HANDBOOK_SECTIONS.map((section) => (
                <button
                  key={section.key}
                  onClick={() => setActiveSection(section.key)}
                  className={`rounded px-3 py-2.5 text-sm font-medium transition-colors ${
                    activeSection === section.key
                      ? 'bg-blue-900 text-white'
                      : 'bg-gray-200 text-gray-700 hover:bg-gray-300 dark:bg-slate-800 dark:text-gray-300 dark:hover:bg-slate-700'
                  }`}
                >
                  {section.label}
                </button>
              ))}
            </div>
          </div>

          {activeContent ? (
            renderMarkdown(activeContent)
          ) : (
            <div className="rounded-sm border border-dashed border-gray-300 px-4 py-8 text-center dark:border-slate-700">
              <Text variant="secondary">No content added for this section yet.</Text>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
