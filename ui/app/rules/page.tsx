'use client';

import { useEffect, useState } from "react";
import { Text } from "../components/Text";

type HandbookSectionKey = 'rules' | 'format' | 'reporting' | 'edit-team';

const HANDBOOK_SECTIONS: { key: HandbookSectionKey; label: string; file: string }[] = [
  { key: 'rules', label: 'Rules', file: '/rules.md' },
  { key: 'format', label: 'Format', file: '/format.md' },
  { key: 'reporting', label: 'Reporting', file: '/reporting.md' },
  { key: 'edit-team', label: 'Edit Team', file: '/edit-team.md' },
];

function renderMarkdown(content: string) {
  const lines = content.split('\n');
  const elements: React.ReactNode[] = [];

  lines.forEach((line, idx) => {
    if (line.startsWith('# ')) {
      elements.push(
        <Text key={idx} as="h1" variant="primary" className="text-3xl font-bold mt-6 mb-4">
          {line.substring(2)}
        </Text>
      );
    } else if (line.startsWith('## ')) {
      elements.push(
        <Text key={idx} as="h2" variant="primary" className="text-2xl font-semibold mt-6 mb-3">
          {line.substring(3)}
        </Text>
      );
    } else if (line.startsWith('### ')) {
      elements.push(
        <Text key={idx} as="h3" variant="primary" className="text-xl font-medium mt-4 mb-2">
          {line.substring(4)}
        </Text>
      );
    } else if (line.startsWith('- ')) {
      const text = line.substring(2);
      const boldMatch = text.match(/^\*\*(.+?)\*\*:\s*(.+)/);
      if (boldMatch) {
        elements.push(
          <li key={idx} className="ml-6 mb-2">
            <Text variant="primary" className="inline">
              <span className="font-bold">{boldMatch[1]}</span>: {boldMatch[2]}
            </Text>
          </li>
        );
      } else {
        elements.push(
          <li key={idx} className="ml-6 mb-2">
            <Text variant="primary">{text}</Text>
          </li>
        );
      }
    } else if (/^\d+\.\s/.test(line)) {
      elements.push(
        <Text key={idx} variant="primary" className="ml-6 mb-2">
          {line}
        </Text>
      );
    } else if (line.startsWith('*') && line.endsWith('*') && !line.startsWith('**')) {
      elements.push(
        <Text key={idx} variant="secondary" className="italic text-sm mt-4">
          {line.substring(1, line.length - 1)}
        </Text>
      );
    } else if (line.trim() === '---') {
      elements.push(<hr key={idx} className="my-6 border-gray-300 dark:border-slate-700" />);
    } else if (line.trim() !== '') {
      elements.push(
        <Text key={idx} variant="primary" className="mb-3">
          {line}
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
  const [contents, setContents] = useState<Record<HandbookSectionKey, string>>({
    rules: '',
    format: '',
    reporting: '',
    'edit-team': '',
  });
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all(
      HANDBOOK_SECTIONS.map(async (section) => {
        const response = await fetch(section.file);
        const text = await response.text();
        return [section.key, text] as const;
      })
    )
      .then((entries) => {
        setContents({
          rules: entries.find(([key]) => key === 'rules')?.[1] ?? '',
          format: entries.find(([key]) => key === 'format')?.[1] ?? '',
          reporting: entries.find(([key]) => key === 'reporting')?.[1] ?? '',
          'edit-team': entries.find(([key]) => key === 'edit-team')?.[1] ?? '',
        });
        setLoading(false);
      })
      .catch(() => setLoading(false));
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
