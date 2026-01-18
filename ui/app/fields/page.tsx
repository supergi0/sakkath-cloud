'use client';

import { useEffect, useState } from "react";
import { MapPin, ExternalLink } from "lucide-react";
import { Text } from "../components/Text";

interface Field {
  id: number;
  name: string;
  hints: string;
  map_link: string;
}

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:9000';

export default function Fields() {
  const [fields, setFields] = useState<Field[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch(`${API_URL}/v1/fields`)
      .then(res => res.json())
      .then(data => {
        setFields(data);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, []);

  if (loading) {
    return (
      <div className="py-4 px-4 md:px-0 min-h-screen">
        <div className="max-w-7xl mx-auto">
          <Text variant="primary">Loading...</Text>
        </div>
      </div>
    );
  }

  return (
    <div className="py-4 px-4 md:px-0 min-h-screen">
      <div className="max-w-7xl mx-auto">
        <div className="rounded-sm p-6 bg-white dark:bg-slate-900">
          <Text as="h1" variant="primary" className="text-xl mb-6">
            Fields
          </Text>

          {fields.length === 0 ? (
            <Text variant="secondary">No fields available.</Text>
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {fields.map((field, index) => (
                <div 
                  key={field.id}
                  className={`p-4 rounded border transition-colors ${
                    index === 0 
                      ? 'border-cyan-900 dark:border-cyan-700 bg-cyan-50 dark:bg-cyan-950/30' 
                      : 'border-gray-200 dark:border-slate-700 hover:border-cyan-900 dark:hover:border-cyan-900'
                  }`}
                >
                  <div className="flex items-start justify-between gap-2 mb-3">
                    <div className="flex items-center gap-2">
                      <div className={`w-8 h-8 rounded-full text-white flex items-center justify-center text-sm font-bold ${
                        index === 0 ? 'bg-cyan-700' : 'bg-cyan-900'
                      }`}>
                        {field.id}
                      </div>
                      <Text variant="primary" className={index === 0 ? "font-bold text-lg" : "font-semibold text-lg"}>
                        {field.name}
                      </Text>
                    </div>
                    {field.map_link && (
                      <a 
                        href={field.map_link}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="p-1.5 rounded hover:bg-gray-100 dark:hover:bg-slate-800 transition-colors"
                        aria-label="Open map"
                      >
                        <ExternalLink className="w-4 h-4 text-cyan-900" />
                      </a>
                    )}
                  </div>
                  
                  {field.hints && (
                    <div className="flex items-start gap-2">
                      <MapPin className="w-4 h-4 text-gray-500 dark:text-gray-400 mt-0.5 flex-shrink-0" />
                      <Text variant="secondary" className="text-sm">
                        {field.hints}
                      </Text>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
