'use client';

import { useEffect, useState, useRef } from "react";
import { useRouter } from "next/navigation";
import { Plus, Trash2, Edit2, Save, X, User, Crown, Shield, Upload, RotateCw } from "lucide-react";
import { Text } from "../components/Text";
import { useAuth } from "../auth-provider";

interface Team {
  id: number;
  name: string;
  location: string | null;
  full_logo: string | null;
  small_logo: string | null;
}

interface Player {
  id: number;
  name: string;
  email: string;
  phone: string | null;
  is_captain: boolean;
  is_spirit_captain: boolean;
}

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:9000';

export default function PocPage() {
  const { isLoggedIn, isPoc, token, roleName, isLoading } = useAuth();
  const router = useRouter();
  const [team, setTeam] = useState<Team | null>(null);
  const [players, setPlayers] = useState<Player[]>([]);
  const [loading, setLoading] = useState(true);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editForm, setEditForm] = useState<Partial<Player>>({});
  const [isAdding, setIsAdding] = useState(false);
  const [newPlayer, setNewPlayer] = useState<Partial<Player>>({ name: '', email: '', phone: '', is_captain: false, is_spirit_captain: false });
  const [isEditingLogo, setIsEditingLogo] = useState(false);
  const [logoPreview, setLogoPreview] = useState<string | null>(null);
  const [rotation, setRotation] = useState(0);
  const [scale, setScale] = useState(1);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isLoading) return;
    
    if (!isLoggedIn) {
      router.push('/login');
      return;
    }
    if (!isPoc && roleName !== 'SUPER' && roleName !== 'ADMIN') {
      router.push('/');
      return;
    }
    fetchData();
  }, [isLoggedIn, isPoc, roleName, router, token, isLoading]);

  const fetchData = async () => {
    if (!token) return;
    try {
      const [teamRes, playersRes] = await Promise.all([
        fetch(`${API_URL}/v1/poc/team`, { headers: { Authorization: `Bearer ${token}` } }),
        fetch(`${API_URL}/v1/poc/players`, { headers: { Authorization: `Bearer ${token}` } }),
      ]);
      if (teamRes.ok) setTeam(await teamRes.json());
      if (playersRes.ok) setPlayers(await playersRes.json());
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const handleEdit = (player: Player) => {
    setEditingId(player.id);
    setEditForm({ ...player });
  };

  const handleSave = async () => {
    if (!editingId || !token) return;
    try {
      const res = await fetch(`${API_URL}/v1/poc/players/${editingId}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify(editForm),
      });
      if (res.ok) {
        setPlayers(players.map(p => p.id === editingId ? { ...p, ...editForm } as Player : p));
        setEditingId(null);
      }
    } catch (err) {
      console.error(err);
    }
  };

  const handleDelete = async (id: number) => {
    if (!token || !confirm('Delete this player?')) return;
    try {
      const res = await fetch(`${API_URL}/v1/poc/players/${id}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${token}` },
      });
      if (res.ok) {
        setPlayers(players.filter(p => p.id !== id));
      }
    } catch (err) {
      console.error(err);
    }
  };

  const handleAdd = async () => {
    if (!token || !newPlayer.name || !newPlayer.email) return;
    try {
      const res = await fetch(`${API_URL}/v1/poc/players`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify(newPlayer),
      });
      if (res.ok) {
        const data = await res.json();
        setPlayers([...players, { id: data.id, ...newPlayer } as Player]);
        setNewPlayer({ name: '', email: '', phone: '', is_captain: false, is_spirit_captain: false });
        setIsAdding(false);
      }
    } catch (err) {
      console.error(err);
    }
  };

  const handleLogoUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    
    const reader = new FileReader();
    reader.onload = (event) => {
      setLogoPreview(event.target?.result as string);
      setIsEditingLogo(true);
    };
    reader.readAsDataURL(file);
  };

  const compressImage = async (dataUrl: string, maxSizeKB: number, quality: number = 0.9): Promise<string> => {
    return new Promise((resolve) => {
      const img = new Image();
      img.onload = () => {
        const canvas = document.createElement('canvas');
        const size = 400;
        canvas.width = size;
        canvas.height = size;
        
        const ctx = canvas.getContext('2d')!;
        
        // Calculate crop dimensions to get a square from the center
        const sourceSize = Math.min(img.width, img.height);
        const sourceX = (img.width - sourceSize) / 2;
        const sourceY = (img.height - sourceSize) / 2;
        
        // Clear canvas
        ctx.clearRect(0, 0, size, size);
        
        // Apply transformations
        ctx.save();
        ctx.translate(size / 2, size / 2);
        ctx.rotate((rotation * Math.PI) / 180);
        ctx.scale(scale, scale);
        
        // Draw the square-cropped image
        ctx.drawImage(
          img,
          sourceX, sourceY, sourceSize, sourceSize,
          -size / 2, -size / 2, size, size
        );
        ctx.restore();
        
        // Create circular mask
        ctx.globalCompositeOperation = 'destination-in';
        ctx.beginPath();
        ctx.arc(size / 2, size / 2, size / 2, 0, Math.PI * 2);
        ctx.fill();
        
        // Compress to target size
        let currentQuality = quality;
        let result = canvas.toDataURL('image/jpeg', currentQuality);
        
        // Base64 is ~1.37x larger than binary, so adjust target
        const targetSize = maxSizeKB * 1024 * 1.37;
        
        while (result.length > targetSize && currentQuality > 0.1) {
          currentQuality -= 0.05;
          result = canvas.toDataURL('image/jpeg', currentQuality);
        }
        
        resolve(result);
      };
      img.src = dataUrl;
    });
  };

  const handleSaveLogo = async () => {
    if (!logoPreview || !token) return;
    
    try {
      const fullLogo = await compressImage(logoPreview, 10, 0.9);
      const smallLogo = await compressImage(logoPreview, 1, 0.5);
      
      const res = await fetch(`${API_URL}/v1/poc/team/logo`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify({ full_logo: fullLogo, small_logo: smallLogo }),
      });
      
      if (res.ok) {
        setTeam(team ? { ...team, full_logo: fullLogo, small_logo: smallLogo } : null);
        setIsEditingLogo(false);
        setLogoPreview(null);
        setRotation(0);
        setScale(1);
      }
    } catch (err) {
      console.error(err);
    }
  };

  if (isLoading || loading) {
    return (
      <div className="py-4 px-4 md:px-0 min-h-screen">
        <div className="max-w-4xl mx-auto">
          <Text variant="primary">Loading...</Text>
        </div>
      </div>
    );
  }

  if (!team) {
    return (
      <div className="py-4 px-4 md:px-0 min-h-screen">
        <div className="max-w-4xl mx-auto">
          <Text variant="primary">You are not assigned to a team.</Text>
        </div>
      </div>
    );
  }

  return (
    <div className="py-4 px-4 md:px-0 min-h-screen">
      <div className="max-w-4xl mx-auto space-y-6">
        {/* Team Info */}
        <div className="rounded-sm p-6 bg-white dark:bg-slate-900">
          <div className="flex items-center gap-4">
            <div className="relative">
              <div className="w-20 h-20 rounded-full bg-gray-100 dark:bg-slate-800 flex items-center justify-center overflow-hidden">
                {team.full_logo ? (
                  <img src={team.full_logo} alt={team.name} className="w-full h-full object-cover" />
                ) : (
                  <Text variant="secondary" className="text-2xl font-bold">{team.name.charAt(0).toUpperCase()}</Text>
                )}
              </div>
              <button
                onClick={() => fileInputRef.current?.click()}
                className="absolute -bottom-1 -right-1 w-8 h-8 rounded-full bg-blue-500 text-white flex items-center justify-center hover:bg-blue-600"
              >
                <Upload className="w-4 h-4" />
              </button>
              <input
                ref={fileInputRef}
                type="file"
                accept="image/*"
                onChange={handleLogoUpload}
                className="hidden"
              />
            </div>
            <div>
              <Text as="h1" variant="primary" className="text-xl font-semibold">{team.name}</Text>
              {team.location && <Text variant="secondary">{team.location}</Text>}
            </div>
          </div>
        </div>

        {/* Logo Editor Modal */}
        {isEditingLogo && logoPreview && (
          <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
            <div className="bg-white dark:bg-slate-900 rounded-lg p-6 max-w-md w-full">
              <Text as="h3" variant="primary" className="text-lg font-semibold mb-4">Edit Team Logo</Text>
              
              <div className="mb-4 flex justify-center">
                <div className="relative w-48 h-48">
                  <div className="w-full h-full rounded-full overflow-hidden bg-gray-100 dark:bg-slate-800 flex items-center justify-center">
                    <img 
                      src={logoPreview} 
                      alt="Preview" 
                      className="w-full h-full object-cover"
                      style={{ 
                        transform: `rotate(${rotation}deg) scale(${scale})`,
                        transformOrigin: 'center'
                      }}
                    />
                  </div>
                </div>
              </div>

              <div className="space-y-4 mb-6">
                <div>
                  <label className="flex items-center justify-between mb-2">
                    <Text variant="secondary" className="text-sm">Rotation</Text>
                    <Text variant="primary" className="text-sm font-medium">{rotation}°</Text>
                  </label>
                  <input
                    type="range"
                    min="0"
                    max="360"
                    value={rotation}
                    onChange={(e) => setRotation(Number(e.target.value))}
                    className="w-full"
                  />
                </div>
                <div>
                  <label className="flex items-center justify-between mb-2">
                    <Text variant="secondary" className="text-sm">Scale</Text>
                    <Text variant="primary" className="text-sm font-medium">{scale.toFixed(1)}x</Text>
                  </label>
                  <input
                    type="range"
                    min="0.5"
                    max="2"
                    step="0.1"
                    value={scale}
                    onChange={(e) => setScale(Number(e.target.value))}
                    className="w-full"
                  />
                </div>
              </div>

              <div className="flex gap-2">
                <button
                  onClick={handleSaveLogo}
                  className="flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded bg-green-500 text-white hover:bg-green-600"
                >
                  <Save className="w-4 h-4" /> Save Logo
                </button>
                <button
                  onClick={() => { setIsEditingLogo(false); setLogoPreview(null); setRotation(0); setScale(1); }}
                  className="flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded bg-gray-200 dark:bg-slate-700 hover:bg-gray-300 dark:hover:bg-slate-600"
                >
                  <X className="w-4 h-4" /> Cancel
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Players Management */}
        <div className="rounded-sm p-6 bg-white dark:bg-slate-900">
          <div className="flex items-center justify-between mb-4">
            <Text as="h2" variant="primary" className="text-lg font-semibold">Players</Text>
            {!isAdding && (
              <button
                onClick={() => setIsAdding(true)}
                className="flex items-center gap-2 px-3 py-2 text-sm rounded bg-blue-500 text-white hover:bg-blue-600"
              >
                <Plus className="w-4 h-4" /> Add Player
              </button>
            )}
          </div>

          {/* Add Player Form */}
          {isAdding && (
            <div className="mb-4 p-4 rounded bg-gray-50 dark:bg-slate-800 border border-gray-200 dark:border-slate-700">
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <input
                  type="text"
                  placeholder="Name"
                  value={newPlayer.name || ''}
                  onChange={e => setNewPlayer({ ...newPlayer, name: e.target.value })}
                  className="px-3 py-2 text-sm rounded bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100"
                />
                <input
                  type="email"
                  placeholder="Email"
                  value={newPlayer.email || ''}
                  onChange={e => setNewPlayer({ ...newPlayer, email: e.target.value })}
                  className="px-3 py-2 text-sm rounded bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100"
                />
                <input
                  type="text"
                  placeholder="Phone"
                  value={newPlayer.phone || ''}
                  onChange={e => setNewPlayer({ ...newPlayer, phone: e.target.value })}
                  className="px-3 py-2 text-sm rounded bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100"
                />
              </div>
              <div className="flex items-center gap-4 mt-3">
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={newPlayer.is_captain || false}
                    onChange={e => setNewPlayer({ ...newPlayer, is_captain: e.target.checked })}
                    className="rounded"
                  />
                  <Text variant="secondary" className="text-sm">Captain (C)</Text>
                </label>
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={newPlayer.is_spirit_captain || false}
                    onChange={e => setNewPlayer({ ...newPlayer, is_spirit_captain: e.target.checked })}
                    className="rounded"
                  />
                  <Text variant="secondary" className="text-sm">Spirit Captain (SC)</Text>
                </label>
              </div>
              <div className="flex gap-2 mt-4">
                <button
                  onClick={handleAdd}
                  className="flex items-center gap-2 px-3 py-2 text-sm rounded bg-green-500 text-white hover:bg-green-600"
                >
                  <Save className="w-4 h-4" /> Save
                </button>
                <button
                  onClick={() => { setIsAdding(false); setNewPlayer({ name: '', email: '', phone: '', is_captain: false, is_spirit_captain: false }); }}
                  className="flex items-center gap-2 px-3 py-2 text-sm rounded bg-gray-200 dark:bg-slate-700 hover:bg-gray-300 dark:hover:bg-slate-600"
                >
                  <X className="w-4 h-4" /> Cancel
                </button>
              </div>
            </div>
          )}

          {/* Player List - Cloudflare DNS Style */}
          <div className="space-y-2">
            {players.map(player => (
              <div
                key={player.id}
                className="flex items-center gap-4 p-4 rounded bg-gray-50 dark:bg-slate-800 border border-gray-200 dark:border-slate-700 hover:border-blue-400 dark:hover:border-blue-500 transition-colors"
              >
                {editingId === player.id ? (
                  <>
                    <div className="flex-1 grid grid-cols-1 md:grid-cols-3 gap-3">
                      <input
                        type="text"
                        value={editForm.name || ''}
                        onChange={e => setEditForm({ ...editForm, name: e.target.value })}
                        className="px-3 py-2 text-sm rounded bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100"
                      />
                      <input
                        type="email"
                        value={editForm.email || ''}
                        onChange={e => setEditForm({ ...editForm, email: e.target.value })}
                        className="px-3 py-2 text-sm rounded bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100"
                      />
                      <input
                        type="text"
                        value={editForm.phone || ''}
                        onChange={e => setEditForm({ ...editForm, phone: e.target.value })}
                        className="px-3 py-2 text-sm rounded bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100"
                      />
                    </div>
                    <div className="flex items-center gap-4">
                      <label className="flex items-center gap-1 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={editForm.is_captain || false}
                          onChange={e => setEditForm({ ...editForm, is_captain: e.target.checked })}
                          className="rounded"
                        />
                        <Text variant="secondary" className="text-xs">C</Text>
                      </label>
                      <label className="flex items-center gap-1 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={editForm.is_spirit_captain || false}
                          onChange={e => setEditForm({ ...editForm, is_spirit_captain: e.target.checked })}
                          className="rounded"
                        />
                        <Text variant="secondary" className="text-xs">SC</Text>
                      </label>
                    </div>
                    <div className="flex gap-2">
                      <button onClick={handleSave} className="p-2 rounded bg-green-500 text-white hover:bg-green-600">
                        <Save className="w-4 h-4" />
                      </button>
                      <button onClick={() => setEditingId(null)} className="p-2 rounded bg-gray-200 dark:bg-slate-700 hover:bg-gray-300 dark:hover:bg-slate-600">
                        <X className="w-4 h-4" />
                      </button>
                    </div>
                  </>
                ) : (
                  <>
                    <User className="w-5 h-5 text-gray-400" />
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <Text variant="primary" className="font-medium truncate">{player.name}</Text>
                        {player.is_captain && (
                          <span className="flex items-center gap-1 px-1.5 py-0.5 text-xs rounded bg-yellow-100 dark:bg-yellow-900 text-yellow-700 dark:text-yellow-300">
                            <Crown className="w-3 h-3" /> C
                          </span>
                        )}
                        {player.is_spirit_captain && (
                          <span className="flex items-center gap-1 px-1.5 py-0.5 text-xs rounded bg-purple-100 dark:bg-purple-900 text-purple-700 dark:text-purple-300">
                            <Shield className="w-3 h-3" /> SC
                          </span>
                        )}
                      </div>
                      <Text variant="secondary" className="text-sm truncate">{player.email}</Text>
                    </div>
                    {player.phone && (
                      <Text variant="secondary" className="text-sm hidden md:block">{player.phone}</Text>
                    )}
                    <div className="flex gap-2">
                      <button onClick={() => handleEdit(player)} className="p-2 rounded hover:bg-gray-200 dark:hover:bg-slate-700">
                        <Edit2 className="w-4 h-4 text-gray-500" />
                      </button>
                      <button onClick={() => handleDelete(player.id)} className="p-2 rounded hover:bg-red-100 dark:hover:bg-red-900/30">
                        <Trash2 className="w-4 h-4 text-red-500" />
                      </button>
                    </div>
                  </>
                )}
              </div>
            ))}
            {players.length === 0 && !isAdding && (
              <Text variant="secondary" className="text-center py-8">No players yet. Add your first player!</Text>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
