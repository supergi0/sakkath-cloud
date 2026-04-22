'use client';

import { useEffect, useState } from "react";
import { MapPin, Navigation } from "lucide-react";
import { Text } from "../components/Text";
import { apiUrl } from "../lib/api";

interface Field {
  id: number;
  name: string;
  hints: string;
  map_link: string;
}

const FIELD_COLORS = [
  'from-emerald-600 to-green-800',
  'from-green-600 to-teal-800',
  'from-lime-600 to-emerald-800',
  'from-teal-600 to-cyan-800',
];

const FIELD_PATTERNS = [
  'radial-gradient(circle at 30% 50%, rgba(255,255,255,0.08) 0%, transparent 50%)',
  'radial-gradient(circle at 70% 40%, rgba(255,255,255,0.08) 0%, transparent 50%)',
  'radial-gradient(circle at 50% 60%, rgba(255,255,255,0.08) 0%, transparent 50%)',
  'radial-gradient(circle at 40% 30%, rgba(255,255,255,0.08) 0%, transparent 50%)',
];

export default function Fields() {
  const [fields, setFields] = useState<Field[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch(apiUrl('/v1/fields'))
      .then(res => res.json())
      .then(data => {
        setFields(data);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, []);

  if (loading) {
    return (
      <div className="py-4 px-4 md:px-0 min-h-screen overflow-x-hidden">
        <div className="max-w-4xl mx-auto">
          <Text variant="primary">Loading...</Text>
        </div>
      </div>
    );
  }

  return (
    <div className="py-4 px-4 md:px-0 min-h-screen overflow-x-hidden">
      <div className="max-w-4xl mx-auto space-y-4">
        {fields.length === 0 ? (
          <Text variant="secondary">No fields available.</Text>
        ) : (
          fields.map((field, index) => (
            <div key={field.id} className="rounded-2xl border border-gray-200 dark:border-slate-800 overflow-hidden bg-white dark:bg-slate-900">
              {/* Field image placeholder */}
              <div
                className={`relative h-40 md:h-48 bg-gradient-to-br ${FIELD_COLORS[index % FIELD_COLORS.length]}`}
                style={{ backgroundImage: FIELD_PATTERNS[index % FIELD_PATTERNS.length] }}
              >
                {/* Field line markings */}
                <div className="absolute inset-4 md:inset-6 border-2 border-white/20 rounded-sm" />
                <div className="absolute top-4 md:top-6 bottom-4 md:bottom-6 left-1/2 -translate-x-px w-0.5 bg-white/20" />
                <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-16 h-16 md:w-20 md:h-20 rounded-full border-2 border-white/20" />
                <div className="absolute bottom-3 right-3 bg-black/40 backdrop-blur-sm rounded-lg px-3 py-1.5">
                  <span className="text-white font-bold text-sm">{field.name}</span>
                </div>
              </div>

              {/* Field info */}
              <div className="p-4">
                {field.hints && (
                  field.map_link ? (
                    <a
                      href={field.map_link}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flex items-start gap-2 group"
                    >
                      <MapPin className="w-4 h-4 text-gray-500 dark:text-gray-400 mt-0.5 shrink-0" />
                      <Text variant="secondary" className="text-sm group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors">
                        {field.hints}
                      </Text>
                      <Navigation className="w-4 h-4 text-gray-400 dark:text-gray-500 mt-0.5 shrink-0 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors" />
                    </a>
                  ) : (
                    <div className="flex items-start gap-2">
                      <MapPin className="w-4 h-4 text-gray-500 dark:text-gray-400 mt-0.5 shrink-0" />
                      <Text variant="secondary" className="text-sm">{field.hints}</Text>
                    </div>
                  )
                )}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
