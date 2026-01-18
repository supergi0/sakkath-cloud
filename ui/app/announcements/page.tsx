'use client';

import { useEffect, useState } from "react";
import { AlertCircle, Bell, Info } from "lucide-react";
import { Text } from "../components/Text";

interface Announcement {
  id: number;
  title: string;
  message: string;
  priority: number;
  created_at: string;
  expires_at: string | null;
}

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:9000';

export default function Announcements() {
  const [announcements, setAnnouncements] = useState<Announcement[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch(`${API_URL}/v1/announcements`)
      .then(res => res.json())
      .then(data => {
        setAnnouncements(data);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, []);

  const getPriorityIcon = (priority: number) => {
    switch (priority) {
      case 0: return <AlertCircle className="w-5 h-5 text-red-500" />;
      case 1: return <Bell className="w-5 h-5 text-yellow-500" />;
      default: return <Info className="w-5 h-5 text-blue-500" />;
    }
  };

  const getPriorityBg = (priority: number) => {
    switch (priority) {
      case 0: return 'border-l-4 border-red-500';
      case 1: return 'border-l-4 border-yellow-500';
      default: return 'border-l-4 border-blue-500';
    }
  };

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    return date.toLocaleDateString('en-IN', { 
      day: 'numeric', 
      month: 'short', 
      hour: '2-digit', 
      minute: '2-digit' 
    });
  };

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
            Announcements
          </Text>

          {announcements.length === 0 ? (
            <Text variant="secondary">No announcements at this time.</Text>
          ) : (
            <div className="space-y-4">
              {announcements.map((item) => (
                <div 
                  key={item.id} 
                  className={`p-4 rounded bg-gray-50 dark:bg-slate-800 ${getPriorityBg(item.priority)}`}
                >
                  <div className="flex items-start gap-3">
                    {getPriorityIcon(item.priority)}
                    <div className="flex-1">
                      <div className="flex items-center justify-between gap-2 mb-1">
                        <Text variant="primary" className="font-semibold">
                          {item.title}
                        </Text>
                        <Text variant="secondary" className="text-xs whitespace-nowrap">
                          {formatDate(item.created_at)}
                        </Text>
                      </div>
                      <Text variant="secondary" className="text-sm">
                        {item.message}
                      </Text>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
