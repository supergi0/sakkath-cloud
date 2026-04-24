'use client';

import { useEffect, useState, useRef } from "react";
import { useRouter } from "next/navigation";
import { Plus, Trash2, Edit2, Save, X, User, Upload, AlertTriangle, ChevronDown, ChevronUp } from "lucide-react";
import { Text } from "../components/Text";
import { useAuth } from "../auth-provider";
import { apiUrl } from "../lib/api";

interface Team {
  id: number;
  name: string;
  abbreviation: string | null;
  location: string | null;
  full_logo: string | null;
  small_logo: string | null;
  roster_moves_remaining: number;
}

interface Player {
  id: number;
  name: string;
  common_name: string | null;
  email: string;
  phone: string | null;
  is_captain: boolean;
  is_spirit_captain: boolean;
}

interface PocMatch {
  id: number;
  t1_id: number;
  t2_id: number;
  t1_name: string;
  t2_name: string;
  t1_score: number;
  t2_score: number;
  t1_spirit: number | null;
  t2_spirit: number | null;
  time: string;
  possession: number | null;
  field_name?: string | null;
}

interface WfdfSpirit {
  rules_knowledge: number;
  fouls_contact: number;
  fair_mindedness: number;
  positive_attitude: number;
  communication: number;
  mvp_player_id: number | null;
  msp_player_id: number | null;
}

interface OpponentPlayer {
  id: number;
  name: string;
  team_id: number;
}

interface SpiritScoreRow {
  id: number;
  match_id: number;
  team_id: number;
  rules_knowledge: number;
  fouls_contact: number;
  fair_mindedness: number;
  positive_attitude: number;
  communication: number;
  total: number;
  mvp_player_id: number | null;
  msp_player_id: number | null;
  submitted_by_team_id: number;
}

interface ScoreConfirmRow {
  id: number;
  match_id: number;
  team_id: number;
  t1_score: number;
  t2_score: number;
}

// Per-match post-game form: opponent spirit + self spirit + score confirm
interface PostMatchForm {
  t1_score: number;
  t2_score: number;
  opponentSpirit: WfdfSpirit;
  selfSpirit: WfdfSpirit;
}

type FeedbackState = { type: 'error' | 'success'; message: string } | null;

const MAX_TEAM_PLAYERS = 22;
type PlayerRole = 'Player' | 'Captain' | 'Spirit Captain';
const PLAYER_ROLES: PlayerRole[] = ['Player', 'Captain', 'Spirit Captain'];

function roleFromPlayer(p: Partial<Player>): PlayerRole {
  if (p.is_captain) return 'Captain';
  if (p.is_spirit_captain) return 'Spirit Captain';
  return 'Player';
}

function roleToFlags(role: PlayerRole) {
  return { is_captain: role === 'Captain', is_spirit_captain: role === 'Spirit Captain' };
}

function normalizeOptionalText(value?: string | null) {
  const trimmed = value?.trim() ?? '';
  return trimmed ? trimmed : null;
}

function consumesRosterMove(original: Player, draft: Partial<Player>, draftRole: PlayerRole) {
  const nextName = draft.name?.trim() ?? '';
  const nextFlags = roleToFlags(draftRole);

  return original.name.trim() !== nextName
    || normalizeOptionalText(original.email) !== normalizeOptionalText(draft.email)
    || normalizeOptionalText(original.phone) !== normalizeOptionalText(draft.phone)
    || original.is_captain !== nextFlags.is_captain
    || original.is_spirit_captain !== nextFlags.is_spirit_captain;
}

function roleBadge(role: PlayerRole) {
  if (role === 'Player') return null;
  const c: Record<string, string> = {
    'Captain': 'bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-300',
    'Spirit Captain': 'bg-purple-100 dark:bg-purple-900/40 text-purple-700 dark:text-purple-300',
  };
  return <span className={`px-1.5 py-0.5 text-[10px] font-semibold rounded ${c[role] || ''}`}>{role}</span>;
}

const defaultSpirit = (): WfdfSpirit => ({ rules_knowledge: 2, fouls_contact: 2, fair_mindedness: 2, positive_attitude: 2, communication: 2, mvp_player_id: null, msp_player_id: null });

function ConfirmDialog({ title, message, onConfirm, onCancel }: { title: string; message: string; onConfirm: () => void; onCancel: () => void }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm px-4">
      <div className="w-full max-w-sm rounded-2xl border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-900 p-6 shadow-xl">
        <div className="flex items-center gap-3 mb-3">
          <AlertTriangle className="h-5 w-5 text-amber-500 shrink-0" />
          <h3 className="text-base font-semibold text-gray-900 dark:text-white">{title}</h3>
        </div>
        <p className="text-sm text-gray-600 dark:text-slate-400 mb-5">{message}</p>
        <div className="flex gap-3 justify-end">
          <button onClick={onCancel} className="rounded-lg px-4 py-2 text-sm font-medium text-gray-700 dark:text-slate-300 bg-gray-100 dark:bg-slate-800 hover:bg-gray-200 dark:hover:bg-slate-700 transition">Cancel</button>
          <button onClick={onConfirm} className="rounded-lg px-4 py-2 text-sm font-semibold text-white bg-amber-500 hover:bg-amber-400 transition">Confirm</button>
        </div>
      </div>
    </div>
  );
}

function SpiritForm({ label, form, onChange, playerList, playerLabel, showMvpMsp = true }: {
  label: string;
  form: WfdfSpirit;
  onChange: (f: WfdfSpirit) => void;
  playerList: { id: number; name: string }[];
  playerLabel: string;
  showMvpMsp?: boolean;
}) {
  const total = form.rules_knowledge + form.fouls_contact + form.fair_mindedness + form.positive_attitude + form.communication;
  const categories = [
    { key: 'rules_knowledge' as const, label: 'Rules Knowledge & Use' },
    { key: 'fouls_contact' as const, label: 'Fouls & Body Contact' },
    { key: 'fair_mindedness' as const, label: 'Fair-Mindedness' },
    { key: 'positive_attitude' as const, label: 'Positive Attitude & Self-Control' },
    { key: 'communication' as const, label: 'Communication' },
  ];

  return (
    <div className="space-y-3">
      <Text variant="primary" className="text-xs font-bold uppercase tracking-wider">{label}</Text>
      {categories.map(({ key, label: catLabel }) => (
        <div key={key}>
          <Text variant="secondary" className="text-xs mb-1">{catLabel}</Text>
          <div className="grid grid-cols-5 gap-1">
            {[0, 1, 2, 3, 4].map(v => (
              <button key={v} onClick={() => onChange({ ...form, [key]: v })}
                className={'py-1.5 text-xs font-semibold rounded-lg border transition-all ' + (
                  form[key] === v
                    ? 'bg-cyan-700 text-white border-cyan-700'
                    : 'border-gray-200 dark:border-slate-600 text-gray-500 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800'
                )}>
                {v}
              </button>
            ))}
          </div>
        </div>
      ))}
      <div className="flex items-center justify-between py-2 px-3 rounded-lg bg-gray-100 dark:bg-slate-800">
        <Text variant="secondary" className="text-xs font-semibold">Total</Text>
        <Text variant="primary" className="text-base font-bold">{total}<span className="text-xs font-normal text-gray-400">/20</span></Text>
      </div>
      {showMvpMsp && (
        <>
          <div>
            <Text variant="secondary" className="text-xs mb-1">MVP ({playerLabel})</Text>
            <select value={form.mvp_player_id ?? ''} onChange={e => onChange({ ...form, mvp_player_id: e.target.value ? Number(e.target.value) : null })}
              className="w-full px-3 py-1.5 text-sm rounded-lg border border-gray-200 dark:border-slate-600 bg-white dark:bg-slate-900 text-gray-900 dark:text-white">
              <option value="">--</option>
              {playerList.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </div>
          <div>
            <Text variant="secondary" className="text-xs mb-1">MSP ({playerLabel})</Text>
            <select value={form.msp_player_id ?? ''} onChange={e => onChange({ ...form, msp_player_id: e.target.value ? Number(e.target.value) : null })}
              className="w-full px-3 py-1.5 text-sm rounded-lg border border-gray-200 dark:border-slate-600 bg-white dark:bg-slate-900 text-gray-900 dark:text-white">
              <option value="">--</option>
              {playerList.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </div>
        </>
      )}
    </div>
  );
}

export default function MyTeamPage() {
  const { isLoggedIn, isPoc, token, roleName, isLoading } = useAuth();
  const router = useRouter();
  const [team, setTeam] = useState<Team | null>(null);
  const [teamAbbreviation, setTeamAbbreviation] = useState('');
  const [players, setPlayers] = useState<Player[]>([]);
  const [matches, setMatches] = useState<PocMatch[]>([]);
  const [savingTeamAbbreviation, setSavingTeamAbbreviation] = useState(false);
  const [loading, setLoading] = useState(true);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editForm, setEditForm] = useState<Partial<Player>>({});
  const [editRole, setEditRole] = useState<PlayerRole>('Player');
  const [isAdding, setIsAdding] = useState(false);
  const [newPlayer, setNewPlayer] = useState<Partial<Player>>({ name: '', common_name: '', email: '', phone: '', is_captain: false, is_spirit_captain: false });
  const [newRole, setNewRole] = useState<PlayerRole>('Player');
  const [isEditingLogo, setIsEditingLogo] = useState(false);
  const [logoPreview, setLogoPreview] = useState<string | null>(null);
  const [rotation, setRotation] = useState(0);
  const [scale, setScale] = useState(1);
  const [postForms, setPostForms] = useState<Record<number, PostMatchForm>>({});
  const [opponentPlayers, setOpponentPlayers] = useState<Record<number, OpponentPlayer[]>>({});
  const [confirmedMatches, setConfirmedMatches] = useState<Set<number>>(new Set());
  const [submittedSpirits, setSubmittedSpirits] = useState<Set<number>>(new Set());
  const [submittedSelfSpirits, setSubmittedSelfSpirits] = useState<Set<number>>(new Set());
  const [otherTeamSpirits, setOtherTeamSpirits] = useState<Set<number>>(new Set());
  const [expandedMatch, setExpandedMatch] = useState<number | null>(null);
  const [confirmDialog, setConfirmDialog] = useState<{ title: string; message: string; onConfirm: () => void } | null>(null);
  const [feedback, setFeedback] = useState<FeedbackState>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const editingPlayer = editingId ? players.find(player => player.id === editingId) ?? null : null;
  const rosterMovesExhausted = !!team && team.roster_moves_remaining <= 0;
  const saveConsumesMove = editingPlayer ? consumesRosterMove(editingPlayer, editForm, editRole) : false;

  useEffect(() => {
    if (isLoading) return;
    if (!isLoggedIn) { router.push('/login'); return; }
    if (!isPoc) {
      router.push(roleName === 'SUPER' || roleName === 'ADMIN' ? '/admin' : '/');
      return;
    }
    fetchData();
  }, [isLoggedIn, isPoc, roleName, router, token, isLoading]);

  const fetchData = async () => {
    if (!token) return;
    try {
      const [teamRes, playersRes, matchesRes] = await Promise.all([
        fetch(apiUrl('/v1/poc/team'), { headers: { Authorization: `Bearer ${token}` } }),
        fetch(apiUrl('/v1/poc/players'), { headers: { Authorization: `Bearer ${token}` } }),
        fetch(apiUrl('/v1/poc/matches'), { headers: { Authorization: `Bearer ${token}` } }),
      ]);
      if (teamRes.ok) {
        const teamData = await teamRes.json();
        setTeam(teamData);
        setTeamAbbreviation(teamData.abbreviation || '');
      }
      if (playersRes.ok) setPlayers(await playersRes.json());
      if (matchesRes.ok) setMatches(await matchesRes.json());
    } catch (err) { console.error(err); }
    finally { setLoading(false); }
  };

  const handleTeamAbbreviationSave = async () => {
    if (!token || !team) return;

    const trimmed = teamAbbreviation.trim();
    if (trimmed && !/^[A-Za-z0-9]{1,5}$/.test(trimmed)) {
      return;
    }

    try {
      setSavingTeamAbbreviation(true);
      setFeedback(null);
      const res = await fetch(apiUrl('/v1/poc/team'), {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify({ abbreviation: trimmed || null }),
      });
      if (res.ok) {
        setTeam({ ...team, abbreviation: trimmed || null });
      } else {
        setFeedback({ type: 'error', message: 'Unable to save the team code right now.' });
      }
    } catch (err) { console.error(err); }
    finally { setSavingTeamAbbreviation(false); }
  };

  const handleEdit = (player: Player) => {
    setEditingId(player.id);
    setEditForm({ ...player });
    setEditRole(roleFromPlayer(player));
  };

  const handleSave = () => {
    if (!editingId || !token || !team) return;
    setConfirmDialog({
      title: 'Save Changes', message: 'Save changes to this player?',
      onConfirm: async () => {
        setConfirmDialog(null);
        setFeedback(null);
        const flags = roleToFlags(editRole);
        const payload = { ...editForm, ...flags };
        try {
          const res = await fetch(apiUrl(`/v1/poc/players/${editingId}`), {
            method: 'PUT', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
            body: JSON.stringify(payload),
          });
          if (res.ok) {
            const data = await res.json();
            setPlayers(players.map(p => p.id === editingId ? { ...p, ...payload } as Player : p));
            setTeam({ ...team, roster_moves_remaining: data.roster_moves_remaining ?? team.roster_moves_remaining });
            setEditingId(null);
          } else if (res.status === 409) {
            setFeedback({ type: 'error', message: 'Roster edit/remove slots are exhausted. Only common name edits stay free now.' });
          } else {
            setFeedback({ type: 'error', message: 'Unable to save this player right now.' });
          }
        } catch (err) { console.error(err); }
      },
    });
  };

  const handleDelete = (id: number, name: string) => {
    if (!token || !team) return;
    setConfirmDialog({
      title: 'Delete Player', message: `Remove ${name} from the team? This cannot be undone.`,
      onConfirm: async () => {
        setConfirmDialog(null);
        setFeedback(null);
        try {
          const res = await fetch(apiUrl(`/v1/poc/players/${id}`), { method: 'DELETE', headers: { Authorization: `Bearer ${token}` } });
          if (res.ok) {
            const data = await res.json();
            setPlayers(players.filter(p => p.id !== id));
            setTeam({ ...team, roster_moves_remaining: data.roster_moves_remaining ?? team.roster_moves_remaining });
          } else if (res.status === 409) {
            setFeedback({ type: 'error', message: 'No roster edit/remove slots remain for this team.' });
          } else {
            setFeedback({ type: 'error', message: 'Unable to remove this player right now.' });
          }
        } catch (err) { console.error(err); }
      },
    });
  };

  const handleAdd = () => {
    if (!token || !newPlayer.name) return;
    if (players.length >= MAX_TEAM_PLAYERS) {
      setFeedback({ type: 'error', message: `This team already has the maximum ${MAX_TEAM_PLAYERS} players.` });
      return;
    }
    setConfirmDialog({
      title: 'Add Player', message: `Add ${newPlayer.name} as ${newRole}?`,
      onConfirm: async () => {
        setConfirmDialog(null);
        setFeedback(null);
        const flags = roleToFlags(newRole);
        const payload = { ...newPlayer, ...flags };
        try {
          const res = await fetch(apiUrl('/v1/poc/players'), {
            method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
            body: JSON.stringify(payload),
          });
          if (res.ok) {
            const data = await res.json();
            setPlayers([...players, { id: data.id, ...payload } as Player]);
            setNewPlayer({ name: '', common_name: '', email: '', phone: '', is_captain: false, is_spirit_captain: false });
            setNewRole('Player'); setIsAdding(false);
          } else if (res.status === 409) {
            setFeedback({ type: 'error', message: `This team already has the maximum ${MAX_TEAM_PLAYERS} players.` });
          } else {
            setFeedback({ type: 'error', message: 'Unable to add this player right now.' });
          }
        } catch (err) { console.error(err); }
      },
    });
  };

  const fetchOpponentPlayers = async (matchId: number) => {
    if (!token || opponentPlayers[matchId]) return;
    try {
      const res = await fetch(apiUrl(`/v1/poc/matches/${matchId}/opponent-players`), { headers: { Authorization: `Bearer ${token}` } });
      if (res.ok) {
        const data = await res.json();
        setOpponentPlayers(prev => ({ ...prev, [matchId]: data }));
      }
    } catch (err) { console.error(err); }
  };

  const handleExpandMatch = (matchId: number, match: PocMatch) => {
    if (expandedMatch === matchId) { setExpandedMatch(null); return; }
    setExpandedMatch(matchId);
    fetchOpponentPlayers(matchId);
    if (!postForms[matchId]) {
      setPostForms(prev => ({
        ...prev,
        [matchId]: {
          t1_score: match.t1_score,
          t2_score: match.t2_score,
          opponentSpirit: defaultSpirit(),
          selfSpirit: defaultSpirit(),
        },
      }));
    }
  };

  const handleSubmitPostMatch = async (matchId: number) => {
    if (!token || !team) return;
    const form = postForms[matchId];
    if (!form) return;
    const m = matches.find(x => x.id === matchId);
    if (!m) return;
    const isT1 = m.t1_id === team.id;
    const opponentId = isT1 ? m.t2_id : m.t1_id;

    try {
      // 1. Confirm score
      if (!confirmedMatches.has(matchId)) {
        const res = await fetch(apiUrl(`/v1/poc/matches/${matchId}/confirm-score`), {
          method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
          body: JSON.stringify({ t1_score: form.t1_score, t2_score: form.t2_score }),
        });
        if (res.ok) setConfirmedMatches(prev => new Set([...prev, matchId]));
        else throw new Error('confirm-score');
      }

      // 2. Submit opponent spirit
      if (!submittedSpirits.has(matchId)) {
        const res = await fetch(apiUrl(`/v1/poc/matches/${matchId}/spirit-wfdf`), {
          method: 'PUT', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
          body: JSON.stringify({ ...form.opponentSpirit, team_id: opponentId }),
        });
        if (res.ok) setSubmittedSpirits(prev => new Set([...prev, matchId]));
        else throw new Error('opponent-spirit');
      }

      // 3. Submit self spirit (no MVP/MSP for self-rating)
      if (!submittedSelfSpirits.has(matchId)) {
        const res = await fetch(apiUrl(`/v1/poc/matches/${matchId}/spirit-wfdf`), {
          method: 'PUT', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
          body: JSON.stringify({
            rules_knowledge: form.selfSpirit.rules_knowledge,
            fouls_contact: form.selfSpirit.fouls_contact,
            fair_mindedness: form.selfSpirit.fair_mindedness,
            positive_attitude: form.selfSpirit.positive_attitude,
            communication: form.selfSpirit.communication,
            mvp_player_id: null,
            msp_player_id: null,
            team_id: team.id,
          }),
        });
        if (res.ok) setSubmittedSelfSpirits(prev => new Set([...prev, matchId]));
        else throw new Error('self-spirit');
      }

      setExpandedMatch(null);
    } catch (err) {
      console.error(err);
      setFeedback({ type: 'error', message: 'Unable to complete the full post-match submission right now.' });
    }
  };

  const fetchExistingSpirits = async () => {
    if (!token || !team) return;
    for (const m of matches.filter(m => m.possession !== null && m.possession >= 3)) {
      try {
        const [spiritRes, confirmRes] = await Promise.all([
          fetch(apiUrl(`/v1/matches/${m.id}/spirits`)),
          fetch(apiUrl(`/v1/matches/${m.id}/score-confirmations`)),
        ]);
        if (spiritRes.ok) {
          const spirits: SpiritScoreRow[] = await spiritRes.json();
          const otherTeamId = m.t1_id === team.id ? m.t2_id : m.t1_id;
          // Our submission rating the opponent
          if (spirits.some(s => s.submitted_by_team_id === team.id && s.team_id === otherTeamId))
            setSubmittedSpirits(prev => new Set([...prev, m.id]));
          // Our submission rating ourselves
          if (spirits.some(s => s.submitted_by_team_id === team.id && s.team_id === team.id))
            setSubmittedSelfSpirits(prev => new Set([...prev, m.id]));
          // Opponent's submission rating us
          if (spirits.some(s => s.submitted_by_team_id === otherTeamId))
            setOtherTeamSpirits(prev => new Set([...prev, m.id]));
        }
        if (confirmRes.ok) {
          const confirms: ScoreConfirmRow[] = await confirmRes.json();
          if (confirms.some(c => c.team_id === team.id))
            setConfirmedMatches(prev => new Set([...prev, m.id]));
        }
      } catch {}
    }
  };

  useEffect(() => {
    if (matches.length > 0 && team) fetchExistingSpirits();
  }, [matches, team]);

  const handleLogoUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (event) => { setLogoPreview(event.target?.result as string); setIsEditingLogo(true); };
    reader.readAsDataURL(file);
  };

  const compressImage = async (dataUrl: string, maxSizeKB: number, quality: number = 0.9): Promise<string> => {
    return new Promise((resolve) => {
      const img = new Image();
      img.onload = () => {
        const canvas = document.createElement('canvas');
        const size = 400;
        canvas.width = size; canvas.height = size;
        const ctx = canvas.getContext('2d')!;
        const sourceSize = Math.min(img.width, img.height);
        const sourceX = (img.width - sourceSize) / 2;
        const sourceY = (img.height - sourceSize) / 2;
        ctx.clearRect(0, 0, size, size);
        ctx.save(); ctx.translate(size / 2, size / 2);
        ctx.rotate((rotation * Math.PI) / 180); ctx.scale(scale, scale);
        ctx.drawImage(img, sourceX, sourceY, sourceSize, sourceSize, -size / 2, -size / 2, size, size);
        ctx.restore();
        ctx.globalCompositeOperation = 'destination-in';
        ctx.beginPath(); ctx.arc(size / 2, size / 2, size / 2, 0, Math.PI * 2); ctx.fill();
        let q = quality;
        let result = canvas.toDataURL('image/jpeg', q);
        const target = maxSizeKB * 1024 * 1.37;
        while (result.length > target && q > 0.1) { q -= 0.05; result = canvas.toDataURL('image/jpeg', q); }
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
      const res = await fetch(apiUrl('/v1/poc/team/logo'), {
        method: 'PUT', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify({ full_logo: fullLogo, small_logo: smallLogo }),
      });
      if (res.ok) {
        setTeam(team ? { ...team, full_logo: fullLogo, small_logo: smallLogo } : null);
        setIsEditingLogo(false); setLogoPreview(null); setRotation(0); setScale(1);
      }
    } catch (err) { console.error(err); }
  };

  if (isLoading || loading) return <div className="py-4 px-3 min-h-screen"><div className="max-w-lg mx-auto"><Text variant="primary">Loading...</Text></div></div>;
  if (!team) return <div className="py-4 px-3 min-h-screen"><div className="max-w-lg mx-auto"><Text variant="primary">You are not assigned to a team.</Text></div></div>;

  const getMatchStatus = (m: PocMatch) => {
    if (m.possession === null) return 'upcoming';
    if (m.possession >= 3) return 'ended';
    return 'live';
  };

  const getMatchAction = (m: PocMatch) => {
    const s = getMatchStatus(m);
    if (s === 'upcoming') return 'upcoming';
    if (s === 'live') return 'live';
    const myDone = submittedSpirits.has(m.id) && submittedSelfSpirits.has(m.id) && confirmedMatches.has(m.id);
    if (!myDone) return 'action';
    if (!otherTeamSpirits.has(m.id)) {
      const isT1 = m.t1_id === team.id;
      return 'waiting:' + (isT1 ? m.t2_name : m.t1_name);
    }
    return 'done';
  };

  return (
    <div className="py-4 px-3 min-h-screen">
      <div className="max-w-lg mx-auto space-y-3">

        {/* Team Header */}
        <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-4">
          <div className="flex items-center gap-3">
            <div className="relative shrink-0">
              <div className="w-12 h-12 rounded-full bg-gray-100 dark:bg-slate-800 flex items-center justify-center overflow-hidden">
                {team.full_logo ? (
                  <img src={team.full_logo} alt={team.name} className="w-full h-full object-cover" />
                ) : (
                  <Text variant="secondary" className="text-lg font-bold">{team.name.charAt(0).toUpperCase()}</Text>
                )}
              </div>
              <button onClick={() => fileInputRef.current?.click()}
                className="absolute -bottom-1 -right-1 w-5 h-5 rounded-full bg-cyan-700 text-white flex items-center justify-center hover:bg-cyan-600">
                <Upload className="w-2.5 h-2.5" />
              </button>
              <input ref={fileInputRef} type="file" accept="image/*" onChange={handleLogoUpload} className="hidden" />
            </div>
            <div className="min-w-0">
              <Text as="h1" variant="primary" className="text-base font-bold truncate">{team.name}</Text>
              {team.location && <Text variant="secondary" className="text-xs">{team.location}</Text>}
            </div>
          </div>
          <div className="mt-3 flex items-center gap-2">
            <input
              type="text"
              value={teamAbbreviation}
              maxLength={5}
              placeholder="Team code"
              onChange={e => setTeamAbbreviation(e.target.value.replace(/[^a-zA-Z0-9]/g, ''))}
              className="w-28 rounded-lg border border-gray-200 bg-white px-3 py-1.5 text-sm uppercase text-gray-900 dark:border-slate-600 dark:bg-slate-900 dark:text-white"
            />
            <button
              onClick={handleTeamAbbreviationSave}
              disabled={savingTeamAbbreviation || (team.abbreviation || '') === teamAbbreviation.trim()}
              className="rounded-lg bg-cyan-700 px-3 py-1.5 text-xs font-semibold text-white transition hover:bg-cyan-600 disabled:opacity-50"
            >
              {savingTeamAbbreviation ? 'Saving' : 'Save code'}
            </button>
            <Text variant="secondary" className="text-[11px]">Up to 5 letters or numbers.</Text>
          </div>
        </div>

        {feedback && (
          <div className={`rounded-2xl border px-4 py-3 text-sm ${
            feedback.type === 'error'
              ? 'border-red-200 bg-red-50 text-red-700 dark:border-red-500/30 dark:bg-red-500/10 dark:text-red-200'
              : 'border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-200'
          }`}>
            {feedback.message}
          </div>
        )}

        {/* Matches - unified tiles */}
        {matches.length > 0 && (
          <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-3 space-y-2">
            {matches.map(match => {
              const isT1 = match.t1_id === team.id;
              const status = getMatchStatus(match);
              const action = getMatchAction(match);
              const isEnded = status === 'ended';
              const isExpanded = expandedMatch === match.id;
              const myTeamName = isT1 ? match.t1_name : match.t2_name;
              const oppTeamName = isT1 ? match.t2_name : match.t1_name;
              const myScore = isT1 ? match.t1_score : match.t2_score;
              const oppScore = isT1 ? match.t2_score : match.t1_score;
              const form = postForms[match.id];
              const opponents = opponentPlayers[match.id] || [];
              const myDone = confirmedMatches.has(match.id) && submittedSpirits.has(match.id) && submittedSelfSpirits.has(match.id);

              const matchDate = match.time
                ? new Date(match.time).toLocaleDateString('en-US', { weekday: 'short', day: 'numeric', month: 'short' })
                : '';
              const matchTime = match.time
                ? new Date(match.time).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false })
                : '';

              const badge = () => {
                switch (action) {
                  case 'upcoming': return <span className="px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider rounded-full bg-gray-100 dark:bg-slate-700 text-gray-500 dark:text-slate-400">Upcoming</span>;
                  case 'live': return <span className="px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider rounded-full bg-red-100 dark:bg-red-900/40 text-red-600 dark:text-red-400 animate-pulse">Live</span>;
                  case 'action': return <span className="px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider rounded-full bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-300">Action Required</span>;
                  case 'done': return <span className="px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider rounded-full bg-green-100 dark:bg-green-900/40 text-green-700 dark:text-green-300">Completed</span>;
                  default:
                    if (action.startsWith('waiting:')) {
                      return <span className="px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider rounded-full bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300">Waiting</span>;
                    }
                    return null;
                }
              };

              return (
                <div key={match.id} className="rounded-xl border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800">
                  <div className="p-3 space-y-2">
                    {/* Date + badge row */}
                    <div className="flex items-center justify-between gap-2">
                      <Text variant="secondary" className="text-[11px]">{matchDate}{matchTime && ' \u00b7 ' + matchTime}{match.field_name && ' \u00b7 ' + match.field_name}</Text>
                      {badge()}
                    </div>

                    {/* Score row: my team vs opponent */}
                    <div className="flex items-center gap-2">
                      <Text variant="primary" className="flex-1 min-w-0 text-sm font-semibold text-cyan-700 dark:text-cyan-400 break-words leading-tight">{myTeamName}</Text>
                      <div className="shrink-0 flex items-center gap-1">
                        <Text variant="primary" className="text-xl font-bold tabular-nums">{status === 'upcoming' ? '-' : myScore}</Text>
                        <Text variant="secondary" className="text-sm">:</Text>
                        <Text variant="primary" className="text-xl font-bold tabular-nums">{status === 'upcoming' ? '-' : oppScore}</Text>
                      </div>
                      <Text variant="primary" className="flex-1 min-w-0 text-sm font-semibold text-right break-words leading-tight">{oppTeamName}</Text>
                    </div>

                    {/* Action waiting info */}
                    {action.startsWith('waiting:') && (
                      <Text variant="secondary" className="text-[11px]">Waiting for {action.split(':')[1]} to complete their submission</Text>
                    )}

                    {/* Buttons row */}
                    <div className="flex gap-2 pt-1">
                      {(status === 'upcoming' || status === 'live') && (
                        <button onClick={() => router.push('/admin?match_id=' + match.id)}
                          className="flex-1 rounded-lg bg-amber-400 py-2 text-sm font-semibold text-gray-900 hover:bg-amber-300 transition">
                          {status === 'live' ? 'Resume Reporting' : 'Start Reporting'}
                        </button>
                      )}
                      {isEnded && !myDone && (
                        <button onClick={() => handleExpandMatch(match.id, match)}
                          className="flex-1 flex items-center justify-center gap-1.5 rounded-lg bg-cyan-700 py-2 text-sm font-semibold text-white hover:bg-cyan-600 transition">
                          {isExpanded ? 'Collapse' : 'Complete Post-Match'}{isExpanded ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
                        </button>
                      )}
                      <button onClick={() => router.push('/matches?match_id=' + match.id)}
                        className="rounded-lg border border-gray-200 dark:border-slate-600 px-3 py-2 text-sm text-gray-600 dark:text-slate-300 hover:bg-gray-100 dark:hover:bg-slate-700 transition">
                        View
                      </button>
                    </div>
                  </div>

                  {/* Expanded post-match form */}
                  {isExpanded && isEnded && form && (
                    <div className="border-t border-gray-200 dark:border-slate-700 p-3 space-y-4">
                      {/* Score confirmation */}
                      {!confirmedMatches.has(match.id) && (
                        <div className="space-y-2">
                          <Text variant="primary" className="text-xs font-bold uppercase tracking-wider">Confirm Score</Text>
                          <div className="flex items-center gap-3">
                            <div className="flex-1 min-w-0">
                              <Text variant="secondary" className="text-[11px] mb-1 block break-words">{match.t1_name}</Text>
                              <input type="number" min="0" inputMode="numeric" value={form.t1_score}
                                onChange={e => setPostForms(prev => ({ ...prev, [match.id]: { ...form, t1_score: parseInt(e.target.value) || 0 } }))}
                                className="w-full px-2 py-1.5 text-sm font-bold text-center rounded-lg border border-gray-200 dark:border-slate-600 bg-white dark:bg-slate-900 text-gray-900 dark:text-white" />
                            </div>
                            <Text variant="secondary" className="text-base font-bold mt-4">-</Text>
                            <div className="flex-1 min-w-0">
                              <Text variant="secondary" className="text-[11px] mb-1 block break-words">{match.t2_name}</Text>
                              <input type="number" min="0" inputMode="numeric" value={form.t2_score}
                                onChange={e => setPostForms(prev => ({ ...prev, [match.id]: { ...form, t2_score: parseInt(e.target.value) || 0 } }))}
                                className="w-full px-2 py-1.5 text-sm font-bold text-center rounded-lg border border-gray-200 dark:border-slate-600 bg-white dark:bg-slate-900 text-gray-900 dark:text-white" />
                            </div>
                          </div>
                        </div>
                      )}

                      {/* Opponent spirit */}
                      {!submittedSpirits.has(match.id) && (
                        <SpiritForm
                          label={`Rate ${oppTeamName}`}
                          form={form.opponentSpirit}
                          onChange={opponentSpirit => setPostForms(prev => ({ ...prev, [match.id]: { ...form, opponentSpirit } }))}
                          playerList={opponents}
                          playerLabel={oppTeamName}
                        />
                      )}

                      {/* Self spirit */}
                      {!submittedSelfSpirits.has(match.id) && (
                        <>
                          <div className="border-t border-gray-200 dark:border-slate-700" />
                          <SpiritForm
                            label={`Rate ${myTeamName} (Self)`}
                            form={form.selfSpirit}
                            onChange={selfSpirit => setPostForms(prev => ({ ...prev, [match.id]: { ...form, selfSpirit } }))}
                            playerList={players.map(p => ({ id: p.id, name: p.name }))}
                            playerLabel={myTeamName}
                            showMvpMsp={false}
                          />
                        </>
                      )}

                      <button onClick={() => handleSubmitPostMatch(match.id)}
                        className="w-full py-2.5 text-sm font-semibold rounded-lg bg-cyan-700 text-white hover:bg-cyan-600 transition">
                        Submit All
                      </button>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}

        {/* Squad */}
        <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-3">
          <div className="flex items-center justify-between mb-2">
            <div>
              <Text as="h2" variant="primary" className="text-sm font-bold uppercase tracking-wider">Squad</Text>
              <Text variant="secondary" className="text-[11px]">
                {players.length}/{MAX_TEAM_PLAYERS} players · {team.roster_moves_remaining} roster edits/removals left
              </Text>
            </div>
            {!isAdding && (
              <button
                onClick={() => { setFeedback(null); setIsAdding(true); }}
                disabled={players.length >= MAX_TEAM_PLAYERS}
                className="flex items-center gap-1 px-2.5 py-1 text-xs rounded-lg bg-cyan-700 text-white hover:bg-cyan-600 font-semibold transition disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Plus className="w-3 h-3" /> Add
              </button>
            )}
          </div>

          <p className="mb-3 text-xs text-gray-600 dark:text-slate-400">
            Emails and phone numbers are not mandatory, but please fill contact details for a few people so they are reachable in case of issues.
          </p>
          <p className="mb-3 text-xs text-gray-600 dark:text-slate-400">
            Common name updates are always free and remain available even after the roster edit/remove budget is exhausted.
          </p>

          {isAdding && (
            <div className="mb-2 p-3 rounded-xl border border-cyan-200 dark:border-cyan-800 bg-cyan-50/50 dark:bg-cyan-900/10 space-y-2">
              <input type="text" placeholder="Name *" value={newPlayer.name || ''} onChange={e => setNewPlayer({ ...newPlayer, name: e.target.value })}
                className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />              <input type="text" placeholder="Common Name (optional)" value={newPlayer.common_name || ''} onChange={e => setNewPlayer({ ...newPlayer, common_name: e.target.value })}
                className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />              <input type="email" placeholder="Email (optional)" value={newPlayer.email || ''} onChange={e => setNewPlayer({ ...newPlayer, email: e.target.value })}
                className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />
              <input type="text" placeholder="Phone (optional)" value={newPlayer.phone || ''} onChange={e => setNewPlayer({ ...newPlayer, phone: e.target.value })}
                className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />
              <select value={newRole} onChange={e => setNewRole(e.target.value as PlayerRole)}
                className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-white">
                {PLAYER_ROLES.map(r => <option key={r} value={r}>{r}</option>)}
              </select>
              <div className="flex gap-2">
                <button onClick={handleAdd} disabled={!newPlayer.name}
                  className="flex-1 flex items-center justify-center gap-1 py-1.5 text-sm rounded-lg bg-cyan-700 text-white hover:bg-cyan-600 font-semibold disabled:opacity-50 transition">
                  <Save className="w-3 h-3" /> Add
                </button>
                <button onClick={() => { setIsAdding(false); setNewPlayer({ name: '', common_name: '', email: '', phone: '', is_captain: false, is_spirit_captain: false }); setNewRole('Player'); }}
                  className="px-3 py-1.5 text-sm rounded-lg border border-gray-200 dark:border-slate-600 text-gray-600 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-800 transition">
                  Cancel
                </button>
              </div>
            </div>
          )}

          <div className="space-y-1">
            {players.map(player => {
              const role = roleFromPlayer(player);
              return (
                <div key={player.id} className="rounded-lg border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800">
                  {editingId === player.id ? (
                    <div className="p-2.5 space-y-2">
                      <input type="text" value={editForm.name || ''} onChange={e => setEditForm({ ...editForm, name: e.target.value })}
                        disabled={rosterMovesExhausted}
                        className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />
                      <input type="text" placeholder="Common Name (optional)" value={editForm.common_name || ''} onChange={e => setEditForm({ ...editForm, common_name: e.target.value })}
                        className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />
                      <input type="email" placeholder="Email (optional)" value={editForm.email || ''} onChange={e => setEditForm({ ...editForm, email: e.target.value })}
                        disabled={rosterMovesExhausted}
                        className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />
                      <input type="text" placeholder="Phone (optional)" value={editForm.phone || ''} onChange={e => setEditForm({ ...editForm, phone: e.target.value })}
                        disabled={rosterMovesExhausted}
                        className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />
                      <select value={editRole} onChange={e => setEditRole(e.target.value as PlayerRole)}
                        disabled={rosterMovesExhausted}
                        className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-white">
                        {PLAYER_ROLES.map(r => <option key={r} value={r}>{r}</option>)}
                      </select>
                      {rosterMovesExhausted && (
                        <Text variant="secondary" className="text-[11px]">
                          Roster edit/remove slots are exhausted. Only the common name field can be changed now.
                        </Text>
                      )}
                      <div className="flex gap-2">
                        <button onClick={handleSave} disabled={!editForm.name?.trim() || (rosterMovesExhausted && saveConsumesMove)} className="flex-1 flex items-center justify-center gap-1 py-1.5 text-sm rounded-lg bg-cyan-700 text-white hover:bg-cyan-600 font-semibold transition disabled:cursor-not-allowed disabled:opacity-50">
                          <Save className="w-3 h-3" /> Save
                        </button>
                        <button onClick={() => setEditingId(null)} className="px-3 py-1.5 text-sm rounded-lg border border-gray-200 dark:border-slate-600 text-gray-600 dark:text-slate-300 transition">Cancel</button>
                      </div>
                    </div>
                  ) : (
                    <div className="flex items-center gap-2 p-2.5">
                      <User className="w-4 h-4 text-gray-400 dark:text-slate-500 shrink-0" />
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-1.5 flex-wrap">
                          <Text variant="primary" className="text-sm font-medium">{player.name}</Text>
                          {roleBadge(role)}
                        </div>
                        <Text variant="secondary" className="text-xs truncate">{player.email || 'No email added'}</Text>
                      </div>
                      <div className="flex gap-0.5 shrink-0">
                        <button
                          onClick={() => handleEdit(player)}
                          className="p-1.5 rounded-lg hover:bg-gray-200 dark:hover:bg-slate-700 transition"
                        >
                          <Edit2 className="w-3.5 h-3.5 text-gray-500" />
                        </button>
                        <button
                          onClick={() => handleDelete(player.id, player.name)}
                          disabled={team.roster_moves_remaining <= 0}
                          className="p-1.5 rounded-lg hover:bg-red-100 dark:hover:bg-red-900/30 transition disabled:cursor-not-allowed disabled:opacity-40"
                        >
                          <Trash2 className="w-3.5 h-3.5 text-red-500" />
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
            {players.length === 0 && !isAdding && <Text variant="secondary" className="text-sm text-center py-4">No players added yet</Text>}
          </div>
        </div>

        {/* Logo Editor Modal */}
        {isEditingLogo && logoPreview && (
          <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
            <div className="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-2xl p-5 max-w-sm w-full">
              <Text as="h3" variant="primary" className="text-base font-semibold mb-4">Edit Team Logo</Text>
              <div className="mb-4 flex justify-center">
                <div className="w-36 h-36 rounded-full overflow-hidden bg-gray-100 dark:bg-slate-800 flex items-center justify-center">
                  <img src={logoPreview} alt="Preview" className="w-full h-full object-cover"
                    style={{ transform: `rotate(${rotation}deg) scale(${scale})`, transformOrigin: 'center' }} />
                </div>
              </div>
              <div className="space-y-3 mb-5">
                <div>
                  <div className="flex items-center justify-between mb-1"><Text variant="secondary" className="text-xs">Rotation</Text><Text variant="primary" className="text-xs">{rotation}&deg;</Text></div>
                  <input type="range" min="0" max="360" value={rotation} onChange={e => setRotation(Number(e.target.value))} className="w-full" />
                </div>
                <div>
                  <div className="flex items-center justify-between mb-1"><Text variant="secondary" className="text-xs">Scale</Text><Text variant="primary" className="text-xs">{scale.toFixed(1)}x</Text></div>
                  <input type="range" min="0.5" max="2" step="0.1" value={scale} onChange={e => setScale(Number(e.target.value))} className="w-full" />
                </div>
              </div>
              <div className="flex gap-2">
                <button onClick={handleSaveLogo} className="flex-1 flex items-center justify-center gap-2 py-2 rounded-lg bg-cyan-700 text-white hover:bg-cyan-600 text-sm font-semibold transition"><Save className="w-4 h-4" /> Save</button>
                <button onClick={() => { setIsEditingLogo(false); setLogoPreview(null); setRotation(0); setScale(1); }}
                  className="flex-1 flex items-center justify-center gap-2 py-2 rounded-lg border border-gray-200 dark:border-slate-600 text-gray-600 dark:text-slate-300 text-sm transition"><X className="w-4 h-4" /> Cancel</button>
              </div>
            </div>
          </div>
        )}

        {confirmDialog && <ConfirmDialog title={confirmDialog.title} message={confirmDialog.message} onConfirm={confirmDialog.onConfirm} onCancel={() => setConfirmDialog(null)} />}
      </div>
    </div>
  );
}
