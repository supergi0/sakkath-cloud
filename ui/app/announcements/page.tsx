'use client';

import { useEffect, useState } from "react";
import { AlertCircle, Bell, Info, Plus, Pencil, Trash2, X } from "lucide-react";
import { Text } from "../components/Text";
import { useAuth } from "../auth-provider";

interface Announcement {
  id: number;
  title: string;
  message: string;
  priority: number;
  created_at: string;
  expires_at: string | null;
}

interface EditingAnnouncement {
  id?: number;
  title: string;
  message: string;
  priority: number;
  expires_at: string;
}

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:9000';

export default function Announcements() {
  const { isSuperAdmin, token } = useAuth();
  const [announcements, setAnnouncements] = useState<Announcement[]>([]);
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState<EditingAnnouncement | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  const fetchAnnouncements = () => {
    fetch(`${API_URL}/v1/announcements`)
      .then(res => res.json())
      .then(data => {
        setAnnouncements(data);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  };

  useEffect(() => {
    fetchAnnouncements();
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

  const handleCreate = () => {
    setIsCreating(true);
    setEditing({ title: '', message: '', priority: 1, expires_at: '' });
  };

  const handleEdit = (item: Announcement) => {
    setIsCreating(false);
    setEditing({
      id: item.id,
      title: item.title,
      message: item.message,
      priority: item.priority,
      expires_at: item.expires_at || '',
    });
  };

  const handleSave = async () => {
    if (!editing || !token) return;
    
    const body = {
      title: editing.title,
      message: editing.message,
      priority: editing.priority,
      expires_at: editing.expires_at || null,
    };

    if (isCreating) {
      await fetch(`${API_URL}/v1/super/announcements`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify(body),
      });
    } else if (editing.id) {
      await fetch(`${API_URL}/v1/super/announcements/${editing.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify(body),
      });
    }
    
    setEditing(null);
    setIsCreating(false);
    fetchAnnouncements();
  };

  const handleDelete = async (id: number) => {
    if (!token || !confirm('Delete this announcement?')) return;
    
    await fetch(`${API_URL}/v1/super/announcements/${id}`, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${token}` },
    });
    fetchAnnouncements();
  };

  if (loading) {
    return (
      <div className="py-4 px-4 md:px-0 min-h-screen overflow-x-hidden">
        <div className="max-w-7xl mx-auto">
          <Text variant="primary">Loading...</Text>
        </div>
      </div>
    );
  }

  return (
    <div className="py-4 px-4 md:px-0 min-h-screen overflow-x-hidden">
      <div className="max-w-7xl mx-auto">
        <div className="rounded-sm p-6 bg-white dark:bg-slate-900">
          <div className="flex items-center justify-between mb-6">
            <Text as="h1" variant="primary" className="text-xl">
              Announcements
            </Text>
            {isSuperAdmin && (
              <button
                onClick={handleCreate}
                className="flex items-center gap-1 px-3 py-1.5 bg-blue-900 text-white rounded text-sm font-medium hover:bg-blue-800"
              >
                <Plus className="w-4 h-4" /> Add
              </button>
            )}
          </div>

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
                        <div className="flex items-center gap-2">
                          <Text variant="secondary" className="text-xs whitespace-nowrap">
                            {formatDate(item.created_at)}
                          </Text>
                          {isSuperAdmin && (
                            <div className="flex items-center gap-1">
                              <button onClick={() => handleEdit(item)} className="p-1 hover:bg-gray-200 dark:hover:bg-slate-700 rounded">
                                <Pencil className="w-4 h-4 text-gray-500" />
                              </button>
                              <button onClick={() => handleDelete(item.id)} className="p-1 hover:bg-gray-200 dark:hover:bg-slate-700 rounded">
                                <Trash2 className="w-4 h-4 text-red-500" />
                              </button>
                            </div>
                          )}
                        </div>
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

      {/* Edit/Create Modal */}
      {editing && (
        <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4" onClick={() => setEditing(null)}>
          <div className="bg-white dark:bg-slate-900 rounded-lg p-6 w-full max-w-md" onClick={e => e.stopPropagation()}>
            <div className="flex items-center justify-between mb-4">
              <Text variant="primary" className="font-semibold">{isCreating ? 'Create' : 'Edit'} Announcement</Text>
              <button onClick={() => setEditing(null)}><X className="w-5 h-5 text-gray-500" /></button>
            </div>
            <div className="space-y-4">
              <div>
                <Text variant="secondary" className="text-sm mb-1">Title</Text>
                <input
                  type="text"
                  value={editing.title}
                  onChange={e => setEditing({ ...editing, title: e.target.value })}
                  className="w-full p-2 rounded border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-gray-900 dark:text-white"
                />
              </div>
              <div>
                <Text variant="secondary" className="text-sm mb-1">Message</Text>
                <textarea
                  value={editing.message}
                  onChange={e => setEditing({ ...editing, message: e.target.value })}
                  rows={3}
                  className="w-full p-2 rounded border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-gray-900 dark:text-white"
                />
              </div>
              <div>
                <Text variant="secondary" className="text-sm mb-1">Priority</Text>
                <select
                  value={editing.priority}
                  onChange={e => setEditing({ ...editing, priority: parseInt(e.target.value) })}
                  className="w-full p-2 rounded border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-gray-900 dark:text-white"
                >
                  <option value={0}>High</option>
                  <option value={1}>Normal</option>
                  <option value={2}>Low</option>
                </select>
              </div>
              <div>
                <Text variant="secondary" className="text-sm mb-1">Expires At (optional)</Text>
                <input
                  type="datetime-local"
                  value={editing.expires_at}
                  onChange={e => setEditing({ ...editing, expires_at: e.target.value })}
                  className="w-full p-2 rounded border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-gray-900 dark:text-white"
                />
              </div>
              <button
                onClick={handleSave}
                className="w-full py-2 bg-blue-900 text-white rounded font-medium hover:bg-blue-800"
              >
                Save
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
