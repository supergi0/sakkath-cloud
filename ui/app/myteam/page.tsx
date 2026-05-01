'use client';

import { useEffect, useState, useRef } from "react";
import { useRouter } from "next/navigation";
import { Plus, Trash2, Edit2, Save, X, User, Upload, AlertTriangle, ChevronDown, ChevronUp, ArrowLeftRight, ChevronLeft, RotateCcw } from "lucide-react";
import { Text } from "../components/Text";
import { useAuth } from "../auth-provider";
import { apiUrl } from "../lib/api";
import { getTeamAbbreviation } from "../lib/team-name";

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
  notes: string;
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
  notes: string | null;
  submitted_by_team_id: number;
}

interface ScoreConfirmRow {
  id: number;
  match_id: number;
  team_id: number;
  t1_score: number;
  t2_score: number;
}

interface ReportingRoundSetting {
  round_key: number;
  label: string;
  is_enabled: boolean;
}

// Per-match post-game form: opponent spirit + self spirit + score confirm
interface PostMatchForm {
  t1_score: number;
  t2_score: number;
  opponentSpirit: WfdfSpirit;
  selfSpirit: WfdfSpirit;
}

interface MockMatchPlayer {
  id: number;
  name: string;
  team_id: number;
}

interface MockMatchEvent {
  id: number;
  player_id: number | null;
  player_name: string;
  team_id: number;
  event_type: number;
  created_at: string;
  action_group: number;
}

interface MockMatchState {
  id: number;
  t1_id: number;
  t2_id: number;
  t1_name: string;
  t2_name: string;
  t1_abbreviation: string | null;
  t2_abbreviation: string | null;
  t1_score: number;
  t2_score: number;
  possession: number | null;
  initial_possession: 1 | 2 | null;
  field_name: string;
  time_label: string;
  players: MockMatchPlayer[];
  events: MockMatchEvent[];
}

type MockMode = 'idle' | 'choose-possession' | 'live' | 'post-match';
type MockPanelKey = 't1' | 'log' | 't2';
type MockConfirmAction = 'start' | 'save' | 'undo' | 'end';

type FeedbackState = { type: 'error' | 'success'; message: string } | null;

const MAX_TEAM_PLAYERS = 22;
const TEAM_EDITS_ROUND_KEY = 10001;
const TEAM_EDITS_LOCKED_MESSAGE = 'Team edits are currently locked by the super admin.';
const MOCK_MATCH_ID = -1;
const MOCK_TEST_TEAM_ID = -99;
const MOCK_EVENT_LABELS = ['Score', 'Assist', 'Block', 'Turnover'];
const MOCK_EVENT_COLORS = ['text-green-500', 'text-sky-400', 'text-violet-400', 'text-amber-400'];
const MOCK_TEST_PLAYERS = [
  'Test One',
  'Test Two',
  'Test Three',
  'Test Four',
  'Test Five',
  'Test Six',
  'Test Seven',
  'Test Eight',
];
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

const countCharacters = (value: string) => value.length;

function getCompactTeamName(name: string, abbreviation?: string | null) {
  if (name.length > 6) {
    return getTeamAbbreviation(name, abbreviation, 6);
  }

  return name;
}

function getHeaderTeamName(name: string, abbreviation?: string | null) {
  if (name.length > 16) {
    return getTeamAbbreviation(name, abbreviation, 6);
  }

  return name;
}

const defaultSpirit = (): WfdfSpirit => ({ rules_knowledge: 2, fouls_contact: 2, fair_mindedness: 2, positive_attitude: 2, communication: 2, mvp_player_id: null, msp_player_id: null, notes: '' });

function buildMockOpponentPlayers(): MockMatchPlayer[] {
  return MOCK_TEST_PLAYERS.map((name, index) => ({
    id: 10000 + index,
    name,
    team_id: MOCK_TEST_TEAM_ID,
  }));
}

function buildInitialMockMatch(team: Team, players: Player[]): MockMatchState {
  const teamPlayers = players.map((player) => ({ id: player.id, name: player.name, team_id: team.id }));
  return {
    id: MOCK_MATCH_ID,
    t1_id: team.id,
    t2_id: MOCK_TEST_TEAM_ID,
    t1_name: team.name,
    t2_name: 'Test Team',
    t1_abbreviation: team.abbreviation,
    t2_abbreviation: 'TEST',
    t1_score: 0,
    t2_score: 0,
    possession: null,
    initial_possession: null,
    field_name: 'Mock Field',
    time_label: 'Mock Match',
    players: [...teamPlayers, ...buildMockOpponentPlayers()],
    events: [],
  };
}

function recomputeMockMatchState(match: MockMatchState, events: MockMatchEvent[]): Pick<MockMatchState, 't1_score' | 't2_score' | 'possession'> {
  let t1Score = 0;
  let t2Score = 0;
  let possession = match.initial_possession;

  for (const event of events) {
    if (possession === null) {
      break;
    }

    if (event.event_type === 0) {
      if (possession === 1) {
        t1Score += 1;
      } else {
        t2Score += 1;
      }
      possession = possession === 1 ? 2 : 1;
    } else if (event.event_type === 2 || event.event_type === 3) {
      possession = possession === 1 ? 2 : 1;
    }
  }

  return { t1_score: t1Score, t2_score: t2Score, possession };
}

function formatMockLogTime(timestamp: string) {
  return new Date(timestamp).toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  });
}

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

function SpiritForm({ label, form, onChange, playerList, playerLabel, showMvpMsp = true, showNotes = false }: {
  label: string;
  form: WfdfSpirit;
  onChange: (f: WfdfSpirit) => void;
  playerList: { id: number; name: string }[];
  playerLabel: string;
  showMvpMsp?: boolean;
  showNotes?: boolean;
}) {
  const total = form.rules_knowledge + form.fouls_contact + form.fair_mindedness + form.positive_attitude + form.communication;
  const noteCharacterCount = countCharacters(form.notes);
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
      {showNotes && (
        <div>
          <div className="mb-1 flex items-center justify-between gap-2">
            <Text variant="secondary" className="text-xs">Notes ({playerLabel})</Text>
            <Text variant="secondary" className={`text-[11px] ${noteCharacterCount > 250 ? 'text-red-600 dark:text-red-400' : ''}`}>{noteCharacterCount}/250 characters</Text>
          </div>
          <textarea
            value={form.notes}
            onChange={e => onChange({ ...form, notes: e.target.value })}
            rows={4}
            placeholder="Optional notes for the other team"
            className="w-full rounded-lg border border-gray-200 px-3 py-2 text-sm text-gray-900 dark:border-slate-600 dark:bg-slate-900 dark:text-white"
          />
        </div>
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
  const [mockMode, setMockMode] = useState<MockMode>('idle');
  const [mockMatch, setMockMatch] = useState<MockMatchState | null>(null);
  const [mockPanel, setMockPanel] = useState<MockPanelKey>('log');
  const [mockPostForm, setMockPostForm] = useState<PostMatchForm | null>(null);
  const [mockConfirmAction, setMockConfirmAction] = useState<MockConfirmAction | null>(null);
  const [mockErrorMessage, setMockErrorMessage] = useState<string | null>(null);
  const [pendingMockPossession, setPendingMockPossession] = useState<1 | 2 | null>(null);
  const [mockPendingScorerId, setMockPendingScorerId] = useState<number | null>(null);
  const [mockPendingAssisterId, setMockPendingAssisterId] = useState<number | null>(null);
  const [mockPendingBlockId, setMockPendingBlockId] = useState<number | null>(null);
  const [mockPendingTurnover, setMockPendingTurnover] = useState(false);
  const [mockPendingTurnoverId, setMockPendingTurnoverId] = useState<number | null>(null);
  const [mockPendingSwitchOnly, setMockPendingSwitchOnly] = useState(false);
  const [confirmDialog, setConfirmDialog] = useState<{ title: string; message: string; onConfirm: () => void } | null>(null);
  const [feedback, setFeedback] = useState<FeedbackState>(null);
  const [teamEditsEnabled, setTeamEditsEnabled] = useState(true);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const editingPlayer = editingId ? players.find(player => player.id === editingId) ?? null : null;
  const rosterMovesExhausted = !!team && team.roster_moves_remaining <= 0;
  const saveConsumesMove = editingPlayer ? consumesRosterMove(editingPlayer, editForm, editRole) : false;
  const teamEditsLocked = !teamEditsEnabled;

  const resetMockComposer = () => {
    setMockPendingScorerId(null);
    setMockPendingAssisterId(null);
    setMockPendingBlockId(null);
    setMockPendingTurnover(false);
    setMockPendingTurnoverId(null);
    setMockPendingSwitchOnly(false);
    setMockErrorMessage(null);
  };

  const resetMockState = () => {
    setMockMode('idle');
    setMockMatch(null);
    setMockPostForm(null);
    setMockConfirmAction(null);
    setPendingMockPossession(null);
    resetMockComposer();
  };

  const ensureMockMatch = () => mockMatch ?? buildInitialMockMatch(team, players);

  const requestMockStart = () => {
    setMockMatch(buildInitialMockMatch(team, players));
    setMockMode('choose-possession');
    setMockConfirmAction(null);
    setPendingMockPossession(null);
    resetMockComposer();
  };

  const startMockReporting = (possession: 1 | 2) => {
    const nextMatch = ensureMockMatch();
    setMockMatch({ ...nextMatch, possession, initial_possession: possession, t1_score: 0, t2_score: 0, events: [] });
    setMockPanel(possession === 1 ? 't1' : 't2');
    setMockMode('live');
    setMockConfirmAction(null);
    setPendingMockPossession(null);
    resetMockComposer();
  };

  const saveMockAction = () => {
    const currentMatch = mockMatch;
    if (!currentMatch || mockPanel === 'log' || currentMatch.possession === null) return;

    const isOffenseView =
      (mockPanel === 't1' && currentMatch.possession === 1) ||
      (mockPanel === 't2' && currentMatch.possession === 2);

    const now = new Date().toISOString();
    const actionGroup = currentMatch.events.length === 0
      ? 1
      : Math.max(...currentMatch.events.map((event) => event.action_group)) + 1;
    let nextEvents = currentMatch.events.slice();
    let nextEventId = currentMatch.events.length === 0
      ? 1
      : Math.max(...currentMatch.events.map((event) => event.id)) + 1;
    const eventTeamId = currentMatch.possession === 1 ? currentMatch.t1_id : currentMatch.t2_id;
    const defenseTeamId = currentMatch.possession === 1 ? currentMatch.t2_id : currentMatch.t1_id;
    const viewedPlayers = currentMatch.players.filter((player) => player.team_id === (mockPanel === 't1' ? currentMatch.t1_id : currentMatch.t2_id));

    if (mockPendingSwitchOnly) {
      setMockMatch({
        ...currentMatch,
        possession: currentMatch.possession === 1 ? 2 : 1,
      });
      resetMockComposer();
      return;
    }

    if (isOffenseView && mockPendingTurnover) {
      nextEvents.push({
        id: nextEventId,
        player_id: null,
        player_name: '',
        team_id: eventTeamId,
        event_type: 3,
        created_at: now,
        action_group: actionGroup,
      });
    } else if (isOffenseView && mockPendingTurnoverId !== null) {
      const player = viewedPlayers.find((entry) => entry.id === mockPendingTurnoverId);
      if (!player) return;
      nextEvents.push({
        id: nextEventId,
        player_id: player.id,
        player_name: player.name,
        team_id: eventTeamId,
        event_type: 3,
        created_at: now,
        action_group: actionGroup,
      });
    } else if (isOffenseView && mockPendingScorerId !== null && mockPendingAssisterId !== null && mockPendingScorerId !== mockPendingAssisterId) {
      const scorer = viewedPlayers.find((entry) => entry.id === mockPendingScorerId);
      const assister = viewedPlayers.find((entry) => entry.id === mockPendingAssisterId);
      if (!scorer || !assister) return;
      nextEvents.push(
        {
          id: nextEventId,
          player_id: assister.id,
          player_name: assister.name,
          team_id: eventTeamId,
          event_type: 1,
          created_at: now,
          action_group: actionGroup,
        },
        {
          id: nextEventId + 1,
          player_id: scorer.id,
          player_name: scorer.name,
          team_id: eventTeamId,
          event_type: 0,
          created_at: now,
          action_group: actionGroup,
        },
      );
    } else if (!isOffenseView && mockPendingBlockId !== null) {
      const blocker = viewedPlayers.find((entry) => entry.id === mockPendingBlockId);
      if (!blocker) return;
      nextEvents.push({
        id: nextEventId,
        player_id: blocker.id,
        player_name: blocker.name,
        team_id: defenseTeamId,
        event_type: 2,
        created_at: now,
        action_group: actionGroup,
      });
    } else {
      return;
    }

    const recomputed = recomputeMockMatchState(currentMatch, nextEvents);
    setMockMatch({ ...currentMatch, ...recomputed, events: nextEvents });
    resetMockComposer();
  };

  const undoMockAction = () => {
    if (!mockMatch || mockMatch.events.length === 0) return;
    const lastGroup = Math.max(...mockMatch.events.map((event) => event.action_group));
    const nextEvents = mockMatch.events.filter((event) => event.action_group !== lastGroup);
    const recomputed = recomputeMockMatchState(mockMatch, nextEvents);
    setMockMatch({ ...mockMatch, ...recomputed, events: nextEvents });
    resetMockComposer();
  };

  const endMockMatch = () => {
    if (!mockMatch) return;
    setMockMatch({ ...mockMatch, possession: 3 });
    setMockPostForm({
      t1_score: mockMatch.t1_score,
      t2_score: mockMatch.t2_score,
      opponentSpirit: defaultSpirit(),
      selfSpirit: defaultSpirit(),
    });
    setMockMode('post-match');
    setMockConfirmAction(null);
    resetMockComposer();
  };

  const handleMockConfirm = () => {
    if (mockConfirmAction === 'start' && pendingMockPossession) {
      startMockReporting(pendingMockPossession);
    } else if (mockConfirmAction === 'save') {
      saveMockAction();
    } else if (mockConfirmAction === 'undo') {
      undoMockAction();
    } else if (mockConfirmAction === 'end') {
      endMockMatch();
    }
    setMockConfirmAction(null);
  };

  const submitMockPostMatch = () => {
    if (!mockPostForm) return;
    if (countCharacters(mockPostForm.opponentSpirit.notes) > 250) {
      setMockErrorMessage('Opponent notes must be 250 characters or fewer.');
      return;
    }
    resetMockState();
  };

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
      const [teamRes, playersRes, matchesRes, settingsRes] = await Promise.all([
        fetch(apiUrl('/v1/poc/team'), { headers: { Authorization: `Bearer ${token}` } }),
        fetch(apiUrl('/v1/poc/players'), { headers: { Authorization: `Bearer ${token}` } }),
        fetch(apiUrl('/v1/poc/matches'), { headers: { Authorization: `Bearer ${token}` } }),
        fetch(apiUrl('/v1/admin/reporting-rounds'), { headers: { Authorization: `Bearer ${token}` } }),
      ]);
      if (teamRes.ok) {
        const teamData = await teamRes.json();
        setTeam(teamData);
        setTeamAbbreviation(teamData.abbreviation || '');
      }
      if (playersRes.ok) setPlayers(await playersRes.json());
      if (matchesRes.ok) setMatches(await matchesRes.json());
      if (settingsRes.ok) {
        const settings: ReportingRoundSetting[] = await settingsRes.json();
        const teamEditSetting = settings.find((setting) => setting.round_key === TEAM_EDITS_ROUND_KEY);
        setTeamEditsEnabled(teamEditSetting ? teamEditSetting.is_enabled : true);
      } else {
        setTeamEditsEnabled(true);
      }
    } catch (err) { console.error(err); }
    finally { setLoading(false); }
  };

  const handleTeamAbbreviationSave = async () => {
    if (!token || !team) return;
    if (teamEditsLocked) {
      setFeedback({ type: 'error', message: TEAM_EDITS_LOCKED_MESSAGE });
      return;
    }

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
      } else if (res.status === 403) {
        setTeamEditsEnabled(false);
        setFeedback({ type: 'error', message: TEAM_EDITS_LOCKED_MESSAGE });
      } else {
        setFeedback({ type: 'error', message: 'Unable to save the team short name right now.' });
      }
    } catch (err) { console.error(err); }
    finally { setSavingTeamAbbreviation(false); }
  };

  const handleEdit = (player: Player) => {
    if (teamEditsLocked) {
      setFeedback({ type: 'error', message: TEAM_EDITS_LOCKED_MESSAGE });
      return;
    }
    setEditingId(player.id);
    setEditForm({ ...player });
    setEditRole(roleFromPlayer(player));
  };

  const handleSave = () => {
    if (!editingId || !token || !team) return;
    if (teamEditsLocked) {
      setFeedback({ type: 'error', message: TEAM_EDITS_LOCKED_MESSAGE });
      return;
    }
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
          } else if (res.status === 403) {
            setTeamEditsEnabled(false);
            setFeedback({ type: 'error', message: TEAM_EDITS_LOCKED_MESSAGE });
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
    if (teamEditsLocked) {
      setFeedback({ type: 'error', message: TEAM_EDITS_LOCKED_MESSAGE });
      return;
    }
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
          } else if (res.status === 403) {
            setTeamEditsEnabled(false);
            setFeedback({ type: 'error', message: TEAM_EDITS_LOCKED_MESSAGE });
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
    if (teamEditsLocked) {
      setFeedback({ type: 'error', message: TEAM_EDITS_LOCKED_MESSAGE });
      return;
    }
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
          } else if (res.status === 403) {
            setTeamEditsEnabled(false);
            setFeedback({ type: 'error', message: TEAM_EDITS_LOCKED_MESSAGE });
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
    if (countCharacters(form.opponentSpirit.notes) > 250) {
      setFeedback({ type: 'error', message: 'Opponent notes must be 250 characters or fewer.' });
      return;
    }

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
    const nextSubmittedSpirits = new Set<number>();
    const nextSubmittedSelfSpirits = new Set<number>();
    const nextOtherTeamSpirits = new Set<number>();
    const nextConfirmedMatches = new Set<number>();

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
            nextSubmittedSpirits.add(m.id);
          // Our submission rating ourselves
          if (spirits.some(s => s.submitted_by_team_id === team.id && s.team_id === team.id))
            nextSubmittedSelfSpirits.add(m.id);
          // Opponent's submission rating us
          if (spirits.some(s => s.submitted_by_team_id === otherTeamId))
            nextOtherTeamSpirits.add(m.id);
        }
        if (confirmRes.ok) {
          const confirms: ScoreConfirmRow[] = await confirmRes.json();
          if (confirms.some(c => c.team_id === team.id))
            nextConfirmedMatches.add(m.id);
        }
      } catch {}
    }

    setSubmittedSpirits(nextSubmittedSpirits);
    setSubmittedSelfSpirits(nextSubmittedSelfSpirits);
    setOtherTeamSpirits(nextOtherTeamSpirits);
    setConfirmedMatches(nextConfirmedMatches);
  };

  useEffect(() => {
    if (matches.length > 0 && team) fetchExistingSpirits();
  }, [matches, team]);

  const handleLogoUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (teamEditsLocked) {
      setFeedback({ type: 'error', message: TEAM_EDITS_LOCKED_MESSAGE });
      e.target.value = '';
      return;
    }
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
    if (teamEditsLocked) {
      setFeedback({ type: 'error', message: TEAM_EDITS_LOCKED_MESSAGE });
      return;
    }
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
      } else if (res.status === 403) {
        setTeamEditsEnabled(false);
        setFeedback({ type: 'error', message: TEAM_EDITS_LOCKED_MESSAGE });
      }
    } catch (err) { console.error(err); }
  };

  if (isLoading || loading) return <div className="py-4 px-3 min-h-screen"><div className="max-w-lg mx-auto"><Text variant="primary">Loading...</Text></div></div>;
  if (!team) return <div className="py-4 px-3 min-h-screen"><div className="max-w-lg mx-auto"><Text variant="primary">You are not assigned to a team.</Text></div></div>;

  if (mockMode === 'choose-possession' && mockMatch) {
    return (
      <div className="min-h-screen px-4 py-4 md:px-0">
        <div className="mx-auto max-w-lg">
          {mockConfirmAction === 'start' && pendingMockPossession && (
            <ConfirmDialog
              title="Start Match"
              message={`Set ${pendingMockPossession === 1 ? mockMatch.t1_name : mockMatch.t2_name} as starting on offense? This can be undone later.`}
              onConfirm={handleMockConfirm}
              onCancel={() => { setMockConfirmAction(null); setPendingMockPossession(null); }}
            />
          )}
          <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6">
            <button
              onClick={resetMockState}
              className="mb-4 inline-flex items-center gap-1.5 text-sm text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white"
            >
              <ChevronLeft className="h-4 w-4" /> Back
            </button>
            <div className="text-lg font-semibold text-gray-900 dark:text-white">{mockMatch.t1_name} vs {mockMatch.t2_name}</div>
            <div className="text-sm text-gray-500 dark:text-slate-400 mt-1">{mockMatch.time_label} &bull; {mockMatch.field_name}</div>
            <div className="mt-6 text-sm font-medium text-gray-700 dark:text-slate-300">Who starts on offense?</div>
            <div className="mt-3 grid grid-cols-2 gap-3">
              <button
                onClick={() => { setPendingMockPossession(1); setMockConfirmAction('start'); }}
                className="rounded-xl border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800 px-4 py-4 text-sm font-semibold text-gray-900 dark:text-white transition hover:border-amber-400 hover:bg-amber-50 dark:hover:bg-amber-400/10"
              >
                {mockMatch.t1_name}
              </button>
              <button
                onClick={() => { setPendingMockPossession(2); setMockConfirmAction('start'); }}
                className="rounded-xl border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800 px-4 py-4 text-sm font-semibold text-gray-900 dark:text-white transition hover:border-amber-400 hover:bg-amber-50 dark:hover:bg-amber-400/10"
              >
                {mockMatch.t2_name}
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  if (mockMode === 'live' && mockMatch) {
    const t1Abbr = getTeamAbbreviation(mockMatch.t1_name, mockMatch.t1_abbreviation, 12);
    const t2Abbr = getTeamAbbreviation(mockMatch.t2_name, mockMatch.t2_abbreviation, 12);
    const displayT1Name = getHeaderTeamName(mockMatch.t1_name, mockMatch.t1_abbreviation);
    const displayT2Name = getHeaderTeamName(mockMatch.t2_name, mockMatch.t2_abbreviation);
    const viewedTeamId = mockPanel === 't1' ? mockMatch.t1_id : mockMatch.t2_id;
    const viewedPlayers = mockMatch.players
      .filter((player) => player.team_id === viewedTeamId)
      .sort((left, right) => left.name.localeCompare(right.name));
    const isOffenseView =
      (mockPanel === 't1' && mockMatch.possession === 1) ||
      (mockPanel === 't2' && mockMatch.possession === 2);
    const saveEnabled = mockPanel !== 'log' && (
      mockPendingSwitchOnly ||
      (isOffenseView && mockPendingTurnover) ||
      (isOffenseView && mockPendingTurnoverId !== null) ||
      (isOffenseView && mockPendingScorerId !== null && mockPendingAssisterId !== null && mockPendingScorerId !== mockPendingAssisterId) ||
      (!isOffenseView && mockPendingBlockId !== null)
    );

    const saveLabel = mockPendingSwitchOnly
      ? 'Switch'
      : (mockPendingTurnover || mockPendingTurnoverId !== null)
        ? 'Save Turnover'
        : isOffenseView
          ? 'Save Score'
          : 'Save Block';

    return (
      <div className="min-h-screen px-3 py-3 md:px-0">
        <div className="mx-auto max-w-2xl space-y-3">
          {mockConfirmAction && (
            <ConfirmDialog
              title={mockConfirmAction === 'end' ? 'End Match' : mockConfirmAction === 'undo' ? 'Undo Event' : 'Save Event'}
              message={
                mockConfirmAction === 'end'
                  ? 'Are you sure you want to end this match? This cannot be easily reversed.'
                  : mockConfirmAction === 'undo'
                    ? 'Undo the last recorded event?'
                    : 'Save this event to the match log?'
              }
              onConfirm={handleMockConfirm}
              onCancel={() => setMockConfirmAction(null)}
            />
          )}

          <div className="flex items-center justify-between">
            <button
              onClick={resetMockState}
              className="inline-flex items-center gap-1.5 rounded-full border border-gray-300 dark:border-slate-700 bg-white dark:bg-slate-900 px-3 py-1.5 text-sm text-gray-700 dark:text-slate-200 transition hover:bg-gray-50 dark:hover:bg-slate-800"
            >
              <ChevronLeft className="h-4 w-4" /> Back
            </button>
            <div className="flex-1 px-3 text-center text-sm font-medium text-gray-500 dark:text-slate-400">Mock Match</div>
            <button
              onClick={() => setMockConfirmAction('end')}
              className="rounded-full bg-red-600 px-5 py-2 text-sm font-semibold text-white transition hover:bg-red-500"
            >
              End Match
            </button>
          </div>

          {mockErrorMessage && (
            <div className="rounded-xl border border-red-300 dark:border-red-500/40 bg-red-50 dark:bg-red-500/10 px-4 py-2">
              <Text className="text-sm text-red-700 dark:text-red-200">{mockErrorMessage}</Text>
            </div>
          )}

          <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-4">
            <div className="grid grid-cols-[1fr_auto_1fr] items-center gap-2">
              <button
                onClick={() => { setMockPanel('t1'); resetMockComposer(); }}
                className={`rounded-xl border p-3 text-left transition ${
                  mockPanel === 't1' ? 'border-amber-400 bg-amber-50 dark:bg-amber-400/10' : 'border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800'
                }`}
              >
                <div className="text-sm font-semibold text-gray-900 dark:text-white truncate" title={mockMatch.t1_name}>{displayT1Name}</div>
                <div className={`text-[10px] font-bold tracking-widest mt-0.5 ${mockMatch.possession === 1 ? 'text-amber-600 dark:text-amber-400' : 'text-sky-600 dark:text-sky-400'}`}>
                  {mockMatch.possession === 1 ? 'OFFENSE' : 'DEFENSE'}
                </div>
                <div className="text-4xl font-black text-gray-900 dark:text-white mt-2">{mockMatch.t1_score}</div>
              </button>
              <div className="text-lg text-gray-400 dark:text-slate-500 font-light">-</div>
              <button
                onClick={() => { setMockPanel('t2'); resetMockComposer(); }}
                className={`rounded-xl border p-3 text-right transition ${
                  mockPanel === 't2' ? 'border-amber-400 bg-amber-50 dark:bg-amber-400/10' : 'border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800'
                }`}
              >
                <div className="text-sm font-semibold text-gray-900 dark:text-white truncate" title={mockMatch.t2_name}>{displayT2Name}</div>
                <div className={`text-[10px] font-bold tracking-widest mt-0.5 ${mockMatch.possession === 2 ? 'text-amber-600 dark:text-amber-400' : 'text-sky-600 dark:text-sky-400'}`}>
                  {mockMatch.possession === 2 ? 'OFFENSE' : 'DEFENSE'}
                </div>
                <div className="text-4xl font-black text-gray-900 dark:text-white mt-2">{mockMatch.t2_score}</div>
              </button>
            </div>
          </div>

          <div className="grid grid-cols-3 rounded-full border border-gray-200 dark:border-slate-800 bg-gray-100 dark:bg-slate-900 p-1">
            {([
              { key: 't1' as MockPanelKey, label: t1Abbr },
              { key: 'log' as MockPanelKey, label: 'Log' },
              { key: 't2' as MockPanelKey, label: t2Abbr },
            ]).map(item => (
              <button
                key={item.key}
                onClick={() => { setMockPanel(item.key); resetMockComposer(); }}
                className={`truncate rounded-full px-2 py-2 text-sm font-semibold transition ${
                  mockPanel === item.key
                    ? 'bg-amber-400 text-gray-900 dark:text-slate-950'
                    : 'text-gray-600 dark:text-slate-400 hover:bg-gray-200 dark:hover:bg-slate-800'
                }`}
              >
                {item.label}
              </button>
            ))}
          </div>

          {mockPanel === 'log' ? (
            <div className="space-y-1.5">
              {mockMatch.events.length === 0 && (
                <Text className="text-sm text-gray-400 dark:text-slate-500 px-1">No events yet</Text>
              )}
              {mockMatch.events.slice().reverse().map(event => {
                const isT1 = event.team_id === mockMatch.t1_id;
                const playerName = event.player_name || (isT1 ? mockMatch.t1_name : mockMatch.t2_name);
                return (
                  <div key={event.id} className={`flex ${isT1 ? 'justify-start' : 'justify-end'}`}>
                    <div className={`inline-flex items-center gap-2 rounded-lg px-3 py-2 ${
                      isT1 ? 'bg-gray-100 dark:bg-slate-800/80' : 'bg-sky-50 dark:bg-sky-500/10'
                    }`}>
                      <span className={`text-xs font-bold ${MOCK_EVENT_COLORS[event.event_type]}`}>{MOCK_EVENT_LABELS[event.event_type]}</span>
                      <span className="text-sm text-gray-900 dark:text-white">{playerName}</span>
                      <span className="text-[10px] text-gray-400 dark:text-slate-500">{formatMockLogTime(event.created_at)}</span>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : isOffenseView ? (
            <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
              <div className="flex items-center justify-between border-b border-gray-200 dark:border-slate-700 px-4 py-3">
                <button
                  onClick={() => setMockConfirmAction('undo')}
                  disabled={mockMatch.events.length === 0}
                  className="inline-flex items-center gap-2 rounded-lg bg-gray-100 dark:bg-slate-800 px-4 py-2.5 text-sm font-semibold text-gray-700 dark:text-slate-200 hover:bg-gray-200 dark:hover:bg-slate-700 disabled:opacity-40 transition"
                >
                  <RotateCcw className="h-4 w-4" /> Undo
                </button>
                <button
                  onClick={() => setMockConfirmAction('save')}
                  disabled={!saveEnabled}
                  className={`inline-flex items-center gap-2 rounded-lg px-5 py-2.5 text-sm font-bold transition ${
                    saveEnabled ? 'bg-amber-400 text-gray-900 hover:bg-amber-300' : 'bg-gray-200 dark:bg-slate-800 text-gray-400 dark:text-slate-500'
                  }`}
                >
                  <Save className="h-4 w-4" /> {saveLabel}
                </button>
              </div>
              <div className="grid grid-cols-[1fr_48px_48px_48px] border-b border-gray-200 dark:border-slate-700 px-4 py-2 text-[10px] font-bold tracking-wider text-gray-500 dark:text-slate-500 uppercase">
                <div>Player</div>
                <div className="text-center">Score</div>
                <div className="text-center">Assist</div>
                <div className="text-center">Turn</div>
              </div>
              {viewedPlayers.map(player => {
                const isScorer = mockPendingScorerId === player.id;
                const isAssister = mockPendingAssisterId === player.id;
                const isTurnover = mockPendingTurnoverId === player.id;
                return (
                  <div
                    key={player.id}
                    className={`grid grid-cols-[1fr_48px_48px_48px] items-center border-b border-gray-100 dark:border-slate-800 px-4 py-2.5 ${
                      isScorer || isAssister || isTurnover ? 'bg-amber-50 dark:bg-amber-400/5' : ''
                    }`}
                  >
                    <div className="text-sm font-medium text-gray-900 dark:text-white truncate pr-2">{player.name}</div>
                    <button
                      onClick={() => {
                        setMockPendingScorerId(prev => prev === player.id ? null : player.id);
                        setMockPendingTurnover(false);
                        setMockPendingTurnoverId(null);
                        setMockPendingSwitchOnly(false);
                      }}
                      className={`mx-auto h-8 w-8 rounded-full text-xs font-bold transition ${
                        isScorer ? 'bg-green-500 text-white ring-2 ring-green-300' : 'bg-gray-100 dark:bg-slate-800 text-gray-400 dark:text-slate-500 hover:bg-green-100 dark:hover:bg-green-900/30'
                      }`}
                    >
                      {isScorer ? 'S' : ''}
                    </button>
                    <button
                      onClick={() => {
                        setMockPendingAssisterId(prev => prev === player.id ? null : player.id);
                        setMockPendingTurnover(false);
                        setMockPendingTurnoverId(null);
                        setMockPendingSwitchOnly(false);
                      }}
                      className={`mx-auto h-8 w-8 rounded-full text-xs font-bold transition ${
                        isAssister ? 'bg-sky-500 text-white ring-2 ring-sky-300' : 'bg-gray-100 dark:bg-slate-800 text-gray-400 dark:text-slate-500 hover:bg-sky-100 dark:hover:bg-sky-900/30'
                      }`}
                    >
                      {isAssister ? 'A' : ''}
                    </button>
                    <button
                      onClick={() => {
                        setMockPendingTurnoverId(prev => prev === player.id ? null : player.id);
                        setMockPendingScorerId(null);
                        setMockPendingAssisterId(null);
                        setMockPendingTurnover(false);
                        setMockPendingSwitchOnly(false);
                      }}
                      className={`mx-auto h-8 w-8 rounded-full text-xs font-bold transition ${
                        isTurnover ? 'bg-amber-500 text-white ring-2 ring-amber-300' : 'bg-gray-100 dark:bg-slate-800 text-gray-400 dark:text-slate-500 hover:bg-amber-100 dark:hover:bg-amber-900/30'
                      }`}
                    >
                      {isTurnover ? 'T' : ''}
                    </button>
                  </div>
                );
              })}
              <button
                onClick={() => {
                  setMockPendingScorerId(null);
                  setMockPendingAssisterId(null);
                  setMockPendingBlockId(null);
                  setMockPendingTurnoverId(null);
                  setMockPendingSwitchOnly(false);
                  setMockPendingTurnover(prev => !prev);
                }}
                className={`w-full grid grid-cols-[1fr_48px_48px_48px] items-center px-4 py-2.5 transition text-left border-b border-gray-100 dark:border-slate-800 ${
                  mockPendingTurnover ? 'bg-amber-100 dark:bg-amber-400/10' : 'hover:bg-gray-50 dark:hover:bg-slate-800'
                }`}
              >
                <span className={`text-sm font-medium ${mockPendingTurnover ? 'text-amber-700 dark:text-amber-300' : 'text-gray-500 dark:text-slate-400'}`}>Turnover (no player)</span>
                <span /><span />
                <span className={`mx-auto h-8 w-8 rounded-full flex items-center justify-center text-xs font-bold ${
                  mockPendingTurnover ? 'bg-amber-400 text-gray-900 ring-2 ring-amber-300' : 'bg-gray-100 dark:bg-slate-800 text-gray-400 dark:text-slate-500'
                }`}>T</span>
              </button>
              <button
                onClick={() => {
                  setMockPendingScorerId(null);
                  setMockPendingAssisterId(null);
                  setMockPendingBlockId(null);
                  setMockPendingTurnover(false);
                  setMockPendingTurnoverId(null);
                  setMockPendingSwitchOnly(prev => !prev);
                }}
                className={`w-full flex items-center justify-between px-4 py-3 transition ${
                  mockPendingSwitchOnly ? 'bg-sky-50 dark:bg-sky-400/10' : 'hover:bg-gray-50 dark:hover:bg-slate-800'
                }`}
              >
                <span className={`text-sm font-medium ${mockPendingSwitchOnly ? 'text-sky-700 dark:text-sky-300' : 'text-gray-500 dark:text-slate-400'}`}>Don&apos;t Know</span>
                <ArrowLeftRight className={`h-4 w-4 ${mockPendingSwitchOnly ? 'text-sky-600 dark:text-sky-300' : 'text-gray-400 dark:text-slate-500'}`} />
              </button>
              {mockPendingScorerId !== null && mockPendingAssisterId !== null && mockPendingScorerId === mockPendingAssisterId && (
                <div className="px-4 py-2 text-sm text-red-600 dark:text-red-300">Scorer and assister must be different</div>
              )}
            </div>
          ) : (
            <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
              <div className="flex items-center justify-between border-b border-gray-200 dark:border-slate-700 px-4 py-3">
                <button
                  onClick={() => setMockConfirmAction('undo')}
                  disabled={mockMatch.events.length === 0}
                  className="inline-flex items-center gap-2 rounded-lg bg-gray-100 dark:bg-slate-800 px-4 py-2.5 text-sm font-semibold text-gray-700 dark:text-slate-200 hover:bg-gray-200 dark:hover:bg-slate-700 disabled:opacity-40 transition"
                >
                  <RotateCcw className="h-4 w-4" /> Undo
                </button>
                <button
                  onClick={() => setMockConfirmAction('save')}
                  disabled={!saveEnabled}
                  className={`inline-flex items-center gap-2 rounded-lg px-5 py-2.5 text-sm font-bold transition ${
                    saveEnabled ? 'bg-amber-400 text-gray-900 hover:bg-amber-300' : 'bg-gray-200 dark:bg-slate-800 text-gray-400 dark:text-slate-500'
                  }`}
                >
                  <Save className="h-4 w-4" /> {saveLabel}
                </button>
              </div>
              <div className="grid grid-cols-[1fr_56px] border-b border-gray-200 dark:border-slate-700 px-4 py-2 text-[10px] font-bold tracking-wider text-gray-500 dark:text-slate-500 uppercase">
                <div>Player</div>
                <div className="text-center">Block</div>
              </div>
              {viewedPlayers.map(player => {
                const isBlock = mockPendingBlockId === player.id;
                return (
                  <div
                    key={player.id}
                    className={`grid grid-cols-[1fr_56px] items-center border-b border-gray-100 dark:border-slate-800 px-4 py-2.5 ${
                      isBlock ? 'bg-violet-50 dark:bg-violet-400/5' : ''
                    }`}
                  >
                    <div className="text-sm font-medium text-gray-900 dark:text-white truncate pr-2">{player.name}</div>
                    <button
                      onClick={() => {
                        setMockPendingBlockId(prev => prev === player.id ? null : player.id);
                        setMockPendingSwitchOnly(false);
                      }}
                      className={`mx-auto h-8 w-8 rounded-full text-xs font-bold transition ${
                        isBlock ? 'bg-violet-500 text-white ring-2 ring-violet-300' : 'bg-gray-100 dark:bg-slate-800 text-gray-400 dark:text-slate-500 hover:bg-violet-100 dark:hover:bg-violet-900/30'
                      }`}
                    >
                      {isBlock ? 'B' : ''}
                    </button>
                  </div>
                );
              })}
              <button
                onClick={() => {
                  setMockPendingScorerId(null);
                  setMockPendingAssisterId(null);
                  setMockPendingBlockId(null);
                  setMockPendingTurnover(false);
                  setMockPendingTurnoverId(null);
                  setMockPendingSwitchOnly(prev => !prev);
                }}
                className={`w-full flex items-center justify-between px-4 py-3 transition ${
                  mockPendingSwitchOnly ? 'bg-sky-50 dark:bg-sky-400/10' : 'hover:bg-gray-50 dark:hover:bg-slate-800'
                }`}
              >
                <span className={`text-sm font-medium ${mockPendingSwitchOnly ? 'text-sky-700 dark:text-sky-300' : 'text-gray-500 dark:text-slate-400'}`}>Don&apos;t Know</span>
                <ArrowLeftRight className={`h-4 w-4 ${mockPendingSwitchOnly ? 'text-sky-600 dark:text-sky-300' : 'text-gray-400 dark:text-slate-500'}`} />
              </button>
            </div>
          )}
        </div>
      </div>
    );
  }

  if (mockMode === 'post-match' && mockMatch && mockPostForm) {
    const myDisplayName = getCompactTeamName(mockMatch.t1_name, mockMatch.t1_abbreviation);
    const oppDisplayName = getCompactTeamName(mockMatch.t2_name, mockMatch.t2_abbreviation);
    const opponentPlayers = mockMatch.players.filter((player) => player.team_id === mockMatch.t2_id);
    const teamPlayers = mockMatch.players.filter((player) => player.team_id === mockMatch.t1_id);

    return (
      <div className="py-4 px-3 min-h-screen">
        <div className="max-w-lg mx-auto space-y-3">
          <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-4 space-y-4">
            <div className="flex items-center justify-between gap-3">
              <button
                onClick={resetMockState}
                className="inline-flex items-center gap-1.5 rounded-full border border-gray-300 dark:border-slate-700 bg-white dark:bg-slate-900 px-3 py-1.5 text-sm text-gray-700 dark:text-slate-200 transition hover:bg-gray-50 dark:hover:bg-slate-800"
              >
                <ChevronLeft className="h-4 w-4" /> Back
              </button>
              <Text variant="primary" className="text-sm font-semibold">Mock Post-Match</Text>
            </div>

            {mockErrorMessage && (
              <div className="rounded-xl border border-red-300 dark:border-red-500/40 bg-red-50 dark:bg-red-500/10 px-4 py-2">
                <Text className="text-sm text-red-700 dark:text-red-200">{mockErrorMessage}</Text>
              </div>
            )}

            <div className="space-y-2">
              <Text variant="primary" className="text-xs font-bold uppercase tracking-wider">Confirm Score</Text>
              <div className="flex items-center gap-3">
                <div className="flex-1 min-w-0">
                  <Text variant="secondary" className="text-[11px] mb-1 block break-words">{myDisplayName}</Text>
                  <input
                    type="number"
                    min="0"
                    inputMode="numeric"
                    value={mockPostForm.t1_score}
                    onChange={e => setMockPostForm({ ...mockPostForm, t1_score: parseInt(e.target.value) || 0 })}
                    className="w-full px-2 py-1.5 text-sm font-bold text-center rounded-lg border border-gray-200 dark:border-slate-600 bg-white dark:bg-slate-900 text-gray-900 dark:text-white"
                  />
                </div>
                <Text variant="secondary" className="text-base font-bold mt-4">-</Text>
                <div className="flex-1 min-w-0">
                  <Text variant="secondary" className="text-[11px] mb-1 block break-words">{oppDisplayName}</Text>
                  <input
                    type="number"
                    min="0"
                    inputMode="numeric"
                    value={mockPostForm.t2_score}
                    onChange={e => setMockPostForm({ ...mockPostForm, t2_score: parseInt(e.target.value) || 0 })}
                    className="w-full px-2 py-1.5 text-sm font-bold text-center rounded-lg border border-gray-200 dark:border-slate-600 bg-white dark:bg-slate-900 text-gray-900 dark:text-white"
                  />
                </div>
              </div>
            </div>

            <SpiritForm
              label={`Rate ${oppDisplayName}`}
              form={mockPostForm.opponentSpirit}
              onChange={opponentSpirit => setMockPostForm({ ...mockPostForm, opponentSpirit })}
              playerList={opponentPlayers}
              playerLabel={oppDisplayName}
              showNotes
            />

            <div className="border-t border-gray-200 dark:border-slate-700" />

            <SpiritForm
              label={`Rate ${myDisplayName} (Self)`}
              form={mockPostForm.selfSpirit}
              onChange={selfSpirit => setMockPostForm({ ...mockPostForm, selfSpirit })}
              playerList={teamPlayers}
              playerLabel={myDisplayName}
              showMvpMsp={false}
            />

            <button
              onClick={submitMockPostMatch}
              className="w-full py-2.5 text-sm font-semibold rounded-lg bg-cyan-700 text-white hover:bg-cyan-600 transition"
            >
              Submit All
            </button>
          </div>
        </div>
      </div>
    );
  }

  const headerTeamName = getHeaderTeamName(team.name, team.abbreviation);

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
      return 'waiting:' + getCompactTeamName(isT1 ? m.t2_name : m.t1_name);
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
                disabled={teamEditsLocked}
                className="absolute -bottom-1 -right-1 flex h-5 w-5 items-center justify-center rounded-full bg-cyan-700 text-white hover:bg-cyan-600 disabled:cursor-not-allowed disabled:opacity-50">
                <Upload className="w-2.5 h-2.5" />
              </button>
              <input ref={fileInputRef} type="file" accept="image/*" onChange={handleLogoUpload} className="hidden" />
            </div>
            <div className="min-w-0">
              <Text as="h1" variant="primary" className="text-base font-bold truncate">{headerTeamName}</Text>
              {team.location && <Text variant="secondary" className="text-xs">{team.location}</Text>}
            </div>
          </div>
          <div className="mt-3 flex items-center gap-2">
            <input
              type="text"
              value={teamAbbreviation}
              maxLength={5}
              placeholder="short name"
              disabled={teamEditsLocked}
              onChange={e => setTeamAbbreviation(e.target.value.replace(/[^a-zA-Z0-9]/g, ''))}
              className="w-28 rounded-lg border border-gray-200 bg-white px-3 py-1.5 text-sm uppercase text-gray-900 disabled:cursor-not-allowed disabled:opacity-50 dark:border-slate-600 dark:bg-slate-900 dark:text-white"
            />
            <button
              onClick={handleTeamAbbreviationSave}
              disabled={teamEditsLocked || savingTeamAbbreviation || (team.abbreviation || '') === teamAbbreviation.trim()}
              className="rounded-lg bg-cyan-700 px-3 py-1.5 text-xs font-semibold text-white transition hover:bg-cyan-600 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {savingTeamAbbreviation ? 'Saving' : 'Save'}
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

        {teamEditsLocked && (
          <div className="rounded-2xl border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-800 dark:border-amber-500/30 dark:bg-amber-400/10 dark:text-amber-200">
            Team edits are locked. Send your request to <a href="mailto:sakkathultimate@gmail.com" className="underline">sakkathultimate@gmail.com</a> for changes.
            
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
              const myDisplayName = getCompactTeamName(myTeamName, team.abbreviation);
              const oppDisplayName = getCompactTeamName(oppTeamName);
              const t1DisplayName = getCompactTeamName(match.t1_name, isT1 ? team.abbreviation : null);
              const t2DisplayName = getCompactTeamName(match.t2_name, !isT1 ? team.abbreviation : null);
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
                      <Text variant="primary" className="flex-1 min-w-0 text-sm font-semibold text-cyan-700 dark:text-cyan-400 break-words leading-tight">{myDisplayName}</Text>
                      <div className="shrink-0 flex items-center gap-1">
                        <Text variant="primary" className="text-xl font-bold tabular-nums">{status === 'upcoming' ? '-' : myScore}</Text>
                        <Text variant="secondary" className="text-sm">:</Text>
                        <Text variant="primary" className="text-xl font-bold tabular-nums">{status === 'upcoming' ? '-' : oppScore}</Text>
                      </div>
                      <Text variant="primary" className="flex-1 min-w-0 text-sm font-semibold text-right break-words leading-tight">{oppDisplayName}</Text>
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
                              <Text variant="secondary" className="text-[11px] mb-1 block break-words">{t1DisplayName}</Text>
                              <input type="number" min="0" inputMode="numeric" value={form.t1_score}
                                onChange={e => setPostForms(prev => ({ ...prev, [match.id]: { ...form, t1_score: parseInt(e.target.value) || 0 } }))}
                                className="w-full px-2 py-1.5 text-sm font-bold text-center rounded-lg border border-gray-200 dark:border-slate-600 bg-white dark:bg-slate-900 text-gray-900 dark:text-white" />
                            </div>
                            <Text variant="secondary" className="text-base font-bold mt-4">-</Text>
                            <div className="flex-1 min-w-0">
                              <Text variant="secondary" className="text-[11px] mb-1 block break-words">{t2DisplayName}</Text>
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
                          label={`Rate ${oppDisplayName}`}
                          form={form.opponentSpirit}
                          onChange={opponentSpirit => setPostForms(prev => ({ ...prev, [match.id]: { ...form, opponentSpirit } }))}
                          playerList={opponents}
                          playerLabel={oppDisplayName}
                          showNotes
                        />
                      )}

                      {/* Self spirit */}
                      {!submittedSelfSpirits.has(match.id) && (
                        <>
                          <div className="border-t border-gray-200 dark:border-slate-700" />
                          <SpiritForm
                            label={`Rate ${myDisplayName} (Self)`}
                            form={form.selfSpirit}
                            onChange={selfSpirit => setPostForms(prev => ({ ...prev, [match.id]: { ...form, selfSpirit } }))}
                            playerList={players.map(p => ({ id: p.id, name: p.name }))}
                            playerLabel={myDisplayName}
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
                disabled={teamEditsLocked || players.length >= MAX_TEAM_PLAYERS}
                className="flex items-center gap-1 px-2.5 py-1 text-xs rounded-lg bg-cyan-700 text-white hover:bg-cyan-600 font-semibold transition disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Plus className="w-3 h-3" /> Add
              </button>
            )}
          </div>

          <p className="mb-3 text-xs text-gray-600 dark:text-slate-400">
            Emails and phone numbers are not mandatory, but please fill contact details for a few people so they are reachable in case of issues.
          </p>

          {isAdding && (
            <div className="mb-2 p-3 rounded-xl border border-cyan-200 dark:border-cyan-800 bg-cyan-50/50 dark:bg-cyan-900/10 space-y-2">
              <input type="text" placeholder="Name *" value={newPlayer.name || ''} onChange={e => setNewPlayer({ ...newPlayer, name: e.target.value })}
                disabled={teamEditsLocked}
                className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />              <input type="text" placeholder="Common Name (optional)" value={newPlayer.common_name || ''} onChange={e => setNewPlayer({ ...newPlayer, common_name: e.target.value })}
                disabled={teamEditsLocked}
                className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />              <input type="email" placeholder="Email (optional)" value={newPlayer.email || ''} onChange={e => setNewPlayer({ ...newPlayer, email: e.target.value })}
                disabled={teamEditsLocked}
                className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />
              <input type="text" placeholder="Phone (optional)" value={newPlayer.phone || ''} onChange={e => setNewPlayer({ ...newPlayer, phone: e.target.value })}
                disabled={teamEditsLocked}
                className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />
              <select value={newRole} onChange={e => setNewRole(e.target.value as PlayerRole)}
                disabled={teamEditsLocked}
                className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-white">
                {PLAYER_ROLES.map(r => <option key={r} value={r}>{r}</option>)}
              </select>
              <div className="flex gap-2">
                <button onClick={handleAdd} disabled={teamEditsLocked || !newPlayer.name}
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
                        disabled={teamEditsLocked || rosterMovesExhausted}
                        className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />
                      <input type="text" placeholder="Common Name (optional)" value={editForm.common_name || ''} onChange={e => setEditForm({ ...editForm, common_name: e.target.value })}
                        disabled={teamEditsLocked}
                        className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />
                      <input type="email" placeholder="Email (optional)" value={editForm.email || ''} onChange={e => setEditForm({ ...editForm, email: e.target.value })}
                        disabled={teamEditsLocked || rosterMovesExhausted}
                        className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />
                      <input type="text" placeholder="Phone (optional)" value={editForm.phone || ''} onChange={e => setEditForm({ ...editForm, phone: e.target.value })}
                        disabled={teamEditsLocked || rosterMovesExhausted}
                        className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-gray-100" />
                      <select value={editRole} onChange={e => setEditRole(e.target.value as PlayerRole)}
                        disabled={teamEditsLocked || rosterMovesExhausted}
                        className="w-full px-3 py-1.5 text-sm rounded-lg bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-white">
                        {PLAYER_ROLES.map(r => <option key={r} value={r}>{r}</option>)}
                      </select>
                      {rosterMovesExhausted && (
                        <Text variant="secondary" className="text-[11px]">
                          Roster edit/remove slots are exhausted. Only the common name field can be changed now.
                        </Text>
                      )}
                      <div className="flex gap-2">
                        <button onClick={handleSave} disabled={teamEditsLocked || !editForm.name?.trim() || (rosterMovesExhausted && saveConsumesMove)} className="flex-1 flex items-center justify-center gap-1 py-1.5 text-sm rounded-lg bg-cyan-700 text-white hover:bg-cyan-600 font-semibold transition disabled:cursor-not-allowed disabled:opacity-50">
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
                          disabled={teamEditsLocked}
                          className="p-1.5 rounded-lg hover:bg-gray-200 dark:hover:bg-slate-700 transition"
                        >
                          <Edit2 className="w-3.5 h-3.5 text-gray-500" />
                        </button>
                        <button
                          onClick={() => handleDelete(player.id, player.name)}
                          disabled={teamEditsLocked || team.roster_moves_remaining <= 0}
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

        <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-3">
          <div className="mb-2">
            <Text as="h2" variant="primary" className="text-sm font-bold uppercase tracking-wider">Mock Match</Text>
          </div>
          <div className="rounded-xl border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800">
            <div className="p-3 space-y-2">
              <div className="flex items-center justify-between gap-2">
                <Text variant="secondary" className="text-[11px]">Mock Match · Mock Field</Text>
                <span className="px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider rounded-full bg-gray-100 dark:bg-slate-700 text-gray-500 dark:text-slate-400">Upcoming</span>
              </div>

              <div className="flex items-center gap-2">
                <Text variant="primary" className="flex-1 min-w-0 text-sm font-semibold text-cyan-700 dark:text-cyan-400 break-words leading-tight">
                  {getCompactTeamName(team.name, team.abbreviation)}
                </Text>
                <div className="shrink-0 flex items-center gap-1">
                  <Text variant="primary" className="text-xl font-bold tabular-nums">-</Text>
                  <Text variant="secondary" className="text-sm">:</Text>
                  <Text variant="primary" className="text-xl font-bold tabular-nums">-</Text>
                </div>
                <Text variant="primary" className="flex-1 min-w-0 text-sm font-semibold text-right break-words leading-tight">
                  {getCompactTeamName('Test Team', 'TEST')}
                </Text>
              </div>

              <div className="flex gap-2 pt-1">
                <button
                  onClick={requestMockStart}
                  className="flex-1 rounded-lg bg-amber-400 py-2 text-sm font-semibold text-gray-900 hover:bg-amber-300 transition"
                >
                  Start Reporting
                </button>
              </div>
            </div>
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
