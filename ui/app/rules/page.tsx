'use client';

import { useEffect, useState } from "react";
import { Text } from "../components/Text";
import { BookOpen } from "lucide-react";

export default function Rules() {
  const [content, setContent] = useState<string>("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch('/rules.md')
      .then(r => r.text())
      .then(text => {
        setContent(text);
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

  // Simple markdown-to-HTML parser for basic formatting
  const renderContent = () => {
    const lines = content.split('\n');
    const elements: React.ReactNode[] = [];
    
    lines.forEach((line, idx) => {
      // Headers
      if (line.startsWith('# ')) {
        elements.push(
          <Text key={idx} as="h1" variant="primary" className="text-3xl font-bold mt-6 mb-4 flex items-center gap-2">
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
        // List item
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
      } else if (line.startsWith('*') && line.endsWith('*') && !line.startsWith('**')) {
        // Italic text (footer)
        elements.push(
          <Text key={idx} variant="secondary" className="italic text-sm mt-4">
            {line.substring(1, line.length - 1)}
          </Text>
        );
      } else if (line.trim() === '---') {
        // Horizontal rule
        elements.push(<hr key={idx} className="my-6 border-gray-300 dark:border-slate-700" />);
      } else if (line.trim() !== '') {
        // Regular paragraph
        elements.push(
          <Text key={idx} variant="primary" className="mb-3">
            {line}
          </Text>
        );
      } else {
        // Empty line for spacing
        elements.push(<div key={idx} className="h-2" />);
      }
    });
    
    return elements;
  };

  return (
    <div className="py-6 px-4 md:px-0 min-h-screen bg-gray-100 dark:bg-slate-950">
      <div className="max-w-4xl mx-auto">
        <div className="rounded-sm p-6 md:p-8 bg-white dark:bg-slate-900">
          {renderContent()}
        </div>
      </div>
    </div>
  );
}
