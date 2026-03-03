'use client';

import { useEffect, useState, useMemo, useRef } from "react";
import { useRouter } from "next/navigation";
import { Circle, Play, ChevronDown } from "lucide-react";
import { Text } from "../components/Text";

interface ScheduleMatch {
  id: number;
  t1_id: number;
  t2_id: number;
  t1_name: string;
  t2_name: string;
  t1_score: number;
  t2_score: number;
  t1_spirit: number | null;
  t2_spirit: number | null;
  t1_small_logo: string | null;
  t2_small_logo: string | null;
  field_name: string;
  time: string;
  possession: number | null;
  stream_url: string | null;
  match_type: number;
}

interface RoundStatus {
  round: number;
  total: number;
  completed: number;
  in_progress: number;
  scheduled: number;
}

interface TournamentState {
  division: number;
  phase: string;
  current_round: number;
  total_rounds: number;
  round_status: RoundStatus[];
}

interface AllData {
  openState: TournamentState | null;
  womenState: TournamentState | null;
  openMatches: ScheduleMatch[];
  womenMatches: ScheduleMatch[];
}

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:9000';

type StatusFilter = 'all' | 'live' | 'upcoming' | 'done';
interface SchedulePreferences {
  division: 0 | 1;
  selectedRound: number | null;
  statusFilter: StatusFilter;
}

const SCHEDULE_PREFS_KEY = 'sakkath:schedule:preferences';

export default function Schedule() {
  const router = useRouter();
  const [division, setDivision] = useState<0 | 1>(0);
  const [selectedRound, setSelectedRound] = useState<number | null>(null);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
  const [allData, setAllData] = useState<AllData | null>(null);
  const [loading, setLoading] = useState(true);
  const [roundDropdownOpen, setRoundDropdownOpen] = useState(false);
  const [prefsReady, setPrefsReady] = useState(false);
  const [hasSavedPreferences, setHasSavedPreferences] = useState(false);
  const [initialRoundSet, setInitialRoundSet] = useState(false);
  const roundDropdownRef = useRef<HTMLDivElement>(null);

  const fetchAllData = async (isInitial = false) => {
    if (isInitial) setLoading(true);

    try {
      const [openState, womenState, openMatches, womenMatches] = await Promise.all([
        fetch(`${API_URL}/v1/schedule/state?division=0`).then(r => r.json()),
        fetch(`${API_URL}/v1/schedule/state?division=1`).then(r => r.json()),
        fetch(`${API_URL}/v1/schedule/matches?division=0`).then(r => r.json()),
        fetch(`${API_URL}/v1/schedule/matches?division=1`).then(r => r.json()),
      ]);

      setAllData({ openState, womenState, openMatches, womenMatches });
    } finally {
      if (isInitial) setLoading(false);
    }
  };

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (roundDropdownRef.current && !roundDropdownRef.current.contains(e.target as Node)) {
        setRoundDropdownOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  // Fetch all data once on mount
  useEffect(() => {
    const savedPreferences = localStorage.getItem(SCHEDULE_PREFS_KEY);
    if (savedPreferences) {
      try {
        const parsed: SchedulePreferences = JSON.parse(savedPreferences);
        if (parsed.division === 0 || parsed.division === 1) {
          setDivision(parsed.division);
        }
        if (parsed.selectedRound === null || typeof parsed.selectedRound === 'number') {
          setSelectedRound(parsed.selectedRound);
        }
        if (parsed.statusFilter === 'all' || parsed.statusFilter === 'live' || parsed.statusFilter === 'upcoming' || parsed.statusFilter === 'done') {
          setStatusFilter(parsed.statusFilter);
        }
        setHasSavedPreferences(true);
      } catch {
      }
    }
    setPrefsReady(true);
  }, []);

  useEffect(() => {
    if (!prefsReady) return;
    const preferences: SchedulePreferences = { division, selectedRound, statusFilter };
    localStorage.setItem(SCHEDULE_PREFS_KEY, JSON.stringify(preferences));
  }, [prefsReady, division, selectedRound, statusFilter]);

  useEffect(() => {
    fetchAllData(true).catch(() => setLoading(false));

    const interval = setInterval(() => {
      fetchAllData(false).catch(() => undefined);
    }, 4000);

    return () => clearInterval(interval);
  }, []);

  // Get current division's data
  const tournamentState = division === 0 ? allData?.openState : allData?.womenState;
  const allMatches = division === 0 ? (allData?.openMatches || []) : (allData?.womenMatches || []);

  const getMatchStatus = (match: ScheduleMatch) => {
    if (match.possession === null) return 'upcoming';
    if (match.possession >= 3) return 'done';
    return 'live';
  };

  // Filter matches client-side (no API call)
  const filteredMatches = useMemo(() => {
    let result = [...allMatches];
    
    // Filter by round
    if (selectedRound !== null) {
      result = result.filter(m => m.match_type === selectedRound);
    }
    
    // Filter by status
    if (statusFilter !== 'all') {
      result = result.filter(m => getMatchStatus(m) === statusFilter);
    }
    
    // Sort by time descending
    return result.sort((a, b) => new Date(b.time).getTime() - new Date(a.time).getTime());
  }, [allMatches, selectedRound, statusFilter]);

  const formatTime = (time: string) => {
    if (!time) return '';
    const d = new Date(time);
    return d.toLocaleDateString('en-US', { weekday: 'short', day: 'numeric', month: 'short' }) + ' - ' + 
           d.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false });
  };

  const getRoundLabel = (type: number) => {
    if (type === 1001) return 'Playoffs';
    if (type === 1002) return 'Finals';
    return `Round ${type}`;
  };

  const currentRoundValue = () => {
    if (!tournamentState) return null;
    if (tournamentState.phase === 'playoffs' || tournamentState.current_round > tournamentState.total_rounds) return 1001;
    if (tournamentState.phase === 'finals') return 1002;
    return tournamentState.current_round;
  };

  const getInitialRoundSelection = () => {
    if (!tournamentState) return null;

    if (tournamentState.phase === 'complete') {
      return null;
    }

    const currentRound = currentRoundValue();
    if (currentRound === null) {
      return null;
    }

    if (currentRound < 1000) {
      const currentStatus = tournamentState.round_status.find((r) => r.round === currentRound);
      const started = (currentStatus?.completed || 0) > 0 || (currentStatus?.in_progress || 0) > 0;
      return started ? currentRound : null;
    }

    const currentMatches = allMatches.filter((m) => m.match_type === currentRound);
    const started = currentMatches.some((m) => m.possession !== null);
    return started ? currentRound : null;
  };

  useEffect(() => {
    if (!prefsReady || !allData || initialRoundSet) return;

    if (hasSavedPreferences) {
      setInitialRoundSet(true);
      return;
    }

    setSelectedRound(getInitialRoundSelection());
    setInitialRoundSet(true);
  }, [prefsReady, allData, initialRoundSet, hasSavedPreferences, division]);

  const getCurrentRoundProgress = () => {
    if (!tournamentState) return null;

    if (tournamentState.phase === 'complete') {
      const finals = allMatches
        .filter((m) => m.match_type === 1002 && m.possession !== null && m.possession >= 3)
        .sort((a, b) => new Date(b.time).getTime() - new Date(a.time).getTime());

      if (finals.length > 0) {
        const final = finals[0];
        const winner = final.t1_score > final.t2_score ? final.t1_name : final.t2_score > final.t1_score ? final.t2_name : null;
        if (winner) return `Tournament complete • Winner: ${winner}`;
      }
      return 'Tournament complete';
    }

    const currentRound = currentRoundValue();
    if (currentRound === null) return null;

    if (currentRound < 1000) {
      const currentStatus = tournamentState.round_status.find((r) => r.round === currentRound);
      if (!currentStatus) return null;

      const started = currentStatus.completed > 0 || currentStatus.in_progress > 0;
      if (!started) {
        return `Tournament not started yet • ${getRoundLabel(currentRound)}`;
      }

      const left = Math.max(currentStatus.total - currentStatus.completed, 0);
      return `Current: ${getRoundLabel(currentRound)} • matches: ${currentStatus.completed} done and ${left} left`;
    }

    const currentMatches = allMatches.filter((m) => m.match_type === currentRound);
    if (currentMatches.length === 0) {
      return currentRound === 1001 ? 'Playoffs pending' : 'Finals pending';
    }

    const completed = currentMatches.filter((m) => m.possession !== null && m.possession >= 3).length;
    const started = currentMatches.some((m) => m.possession !== null);
    if (!started) {
      return `${getRoundLabel(currentRound)} not started yet`;
    }

    const left = Math.max(currentMatches.length - completed, 0);
    return `Current: ${getRoundLabel(currentRound)} • matches: ${completed} done and ${left} left`;
  };

  // Handle division change - reset filters
  const handleDivisionChange = (newDiv: 0 | 1) => {
    setDivision(newDiv);
    setSelectedRound(null);
  };

  if (loading) {
    return (
      <div className="py-4 px-4 md:px-0 min-h-screen bg-gray-100 dark:bg-slate-950">
        <div className="max-w-7xl mx-auto">
          <Text variant="primary">Loading...</Text>
        </div>
      </div>
    );
  }

  // Build round options: All Rounds, Round 1-N, Playoffs
  const roundOptions: { value: number; label: string; status: RoundStatus | null }[] = [];
  const roundStatus = tournamentState?.round_status || [];
  
  // Add regular rounds
  roundStatus.filter(r => r.round < 1000).forEach(r => {
    roundOptions.push({ value: r.round, label: `Round ${r.round}`, status: r });
  });
  
  // Add playoffs if exists
  if (allMatches.some(m => m.match_type === 1001)) {
    roundOptions.push({ value: 1001, label: 'Playoffs', status: null });
  }
  
  // Add finals if exists
  if (allMatches.some(m => m.match_type === 1002)) {
    roundOptions.push({ value: 1002, label: 'Finals', status: null });
  }

  const getSelectedRoundLabel = () => {
    if (selectedRound === null) return 'All Rounds';
    if (selectedRound === 1001) return 'Playoffs';
    if (selectedRound === 1002) return 'Finals';
    return `Round ${selectedRound}`;
  };

  return (
    <div className="min-h-screen bg-gray-100 dark:bg-slate-950">
      {/* Sticky Header - seamless with navbar, solid background to prevent see-through */}
      <div className="sticky top-[60px] md:top-[68px] z-40 bg-gray-100 dark:bg-slate-950">
        <div className="max-w-7xl mx-auto px-4 pt-3 pb-3 bg-gray-100 dark:bg-slate-950 border-b border-gray-200 dark:border-slate-800">
          {/* Row 1: Division Toggle + Round Selector */}
          <div className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-2">
              <button
                onClick={() => handleDivisionChange(0)}
                className={`px-4 py-2 rounded text-sm font-medium transition-colors ${
                  division === 0
                    ? 'bg-blue-900 text-white'
                    : 'bg-gray-200 dark:bg-slate-800 text-gray-700 dark:text-gray-300'
                }`}
              >
                Open
              </button>
              <button
                onClick={() => handleDivisionChange(1)}
                className={`px-4 py-2 rounded text-sm font-medium transition-colors ${
                  division === 1
                    ? 'bg-blue-900 text-white'
                    : 'bg-gray-200 dark:bg-slate-800 text-gray-700 dark:text-gray-300'
                }`}
              >
                Women
              </button>
            </div>
            
            {/* Round dropdown - positioned right, compact width, opens downward */}
            <div className="relative" ref={roundDropdownRef}>
              <button
                onClick={() => setRoundDropdownOpen(!roundDropdownOpen)}
                className="flex items-center gap-1 px-3 py-2 text-sm rounded bg-gray-200 dark:bg-slate-800 text-gray-700 dark:text-gray-300 whitespace-nowrap"
              >
                <span>{getSelectedRoundLabel()}</span>
                <ChevronDown className={`w-4 h-4 transition-transform ${roundDropdownOpen ? 'rotate-180' : ''}`} />
              </button>
              {roundDropdownOpen && (
                <div className="absolute right-0 top-full mt-1 bg-white dark:bg-slate-800 rounded shadow-lg border border-gray-200 dark:border-slate-700 py-1 min-w-[140px] z-50">
                  <button
                    onClick={() => { setSelectedRound(null); setRoundDropdownOpen(false); }}
                    className={`w-full px-3 py-1.5 text-left text-sm hover:bg-gray-100 dark:hover:bg-slate-700 ${
                      selectedRound === null ? 'bg-blue-100 dark:bg-blue-900 text-blue-900 dark:text-blue-100' : 'text-gray-700 dark:text-gray-300'
                    }`}
                  >
                    All Rounds
                  </button>
                  {roundOptions.map(opt => (
                    <button
                      key={opt.value}
                      onClick={() => { setSelectedRound(opt.value); setRoundDropdownOpen(false); }}
                      className={`w-full px-3 py-1.5 text-left text-sm hover:bg-gray-100 dark:hover:bg-slate-700 ${
                        selectedRound === opt.value ? 'bg-blue-100 dark:bg-blue-900 text-blue-900 dark:text-blue-100' : 'text-gray-700 dark:text-gray-300'
                      }`}
                    >
                      {opt.label}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Row 2: Status Filter Buttons - flex full width on mobile */}
          <div className="flex gap-2 mt-3 w-full">
            {(['all', 'live', 'upcoming', 'done'] as StatusFilter[]).map((s) => (
              <button
                key={s}
                onClick={() => setStatusFilter(s)}
                className={`flex-1 md:flex-none md:px-4 py-2 rounded text-sm font-medium transition-colors ${
                  statusFilter === s
                    ? 'bg-blue-900 text-white'
                    : 'bg-gray-200 dark:bg-slate-800 text-gray-600 dark:text-gray-400'
                }`}
              >
                {s.charAt(0).toUpperCase() + s.slice(1)}
              </button>
            ))}
          </div>

          {getCurrentRoundProgress() && (
            <div className="mt-3">
              <span className="inline-flex items-center px-2 py-1 rounded text-xs font-medium bg-blue-100 dark:bg-blue-900 text-blue-900 dark:text-blue-100">
                {getCurrentRoundProgress()}
              </span>
            </div>
          )}
        </div>
      </div>

      {/* Content */}
      <div className="max-w-7xl mx-auto px-4 py-4">
        {filteredMatches.length === 0 ? (
          <div className="rounded-sm p-6 bg-white dark:bg-slate-900 text-center">
            <Text variant="secondary">No matches found</Text>
          </div>
        ) : (
          <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
            {filteredMatches.map((match) => {
              const status = getMatchStatus(match);
              
              return (
                <div 
                  key={match.id} 
                  className={`p-4 rounded bg-white dark:bg-slate-900 border cursor-pointer transition-colors hover:border-blue-500 ${
                    match.match_type === currentRoundValue()
                      ? 'border-blue-500 dark:border-blue-400 ring-1 ring-blue-200 dark:ring-blue-900'
                      : 'border-gray-200 dark:border-slate-700'
                  }`}
                  onClick={() => router.push(`/matches?match_id=${match.id}`)}
                >
                  {/* Match header with round, time, status */}
                  <div className="flex items-center justify-between mb-3">
                    <Text as="div" variant="secondary" className="text-xs font-medium">{`${getRoundLabel(match.match_type)} ${formatTime(match.time)}`}</Text>
                    <div className="flex items-center gap-2">
                      {status === 'live' && (
                        <>
                          <Circle className="w-2 h-2 fill-red-500 text-red-500" />
                          <Text variant="primary" className="text-xs font-medium text-red-500">Live</Text>
                        </>
                      )}
                      {status === 'done' && (
                        <Text variant="secondary" className="text-xs">Ended</Text>
                      )}
                      {status === 'upcoming' && (
                        <>
                          <Circle className="w-2 h-2 fill-yellow-400 text-yellow-400" />
                          <Text variant="primary" className="text-xs font-medium text-yellow-500">Upcoming</Text>
                        </>
                      )}
                      {match.stream_url && (
                        <a 
                          href={match.stream_url} 
                          target="_blank" 
                          rel="noopener noreferrer" 
                          className="text-blue-500 hover:text-blue-400"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <Play className="w-4 h-4" />
                        </a>
                      )}
                    </div>
                  </div>
                  
                  {/* Teams and scores */}
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <div className="w-6 h-6 rounded-full bg-blue-900 text-white flex items-center justify-center text-xs font-bold flex-shrink-0 overflow-hidden">
                          {match.t1_small_logo ? (
                            <img src={match.t1_small_logo} alt="" className="w-full h-full object-cover" />
                          ) : (
                            match.t1_name.charAt(0)
                          )}
                        </div>
                        <Text variant="primary" className="font-medium text-sm truncate max-w-[140px]">{match.t1_name}</Text>
                      </div>
                      <Text variant="primary" className="font-bold text-lg">{status === 'upcoming' ? '-' : match.t1_score}</Text>
                    </div>
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <div className="w-6 h-6 rounded-full bg-blue-900 text-white flex items-center justify-center text-xs font-bold flex-shrink-0 overflow-hidden">
                          {match.t2_small_logo ? (
                            <img src={match.t2_small_logo} alt="" className="w-full h-full object-cover" />
                          ) : (
                            match.t2_name.charAt(0)
                          )}
                        </div>
                        <Text variant="secondary" className="text-sm truncate max-w-[140px]">{match.t2_name}</Text>
                      </div>
                      <Text variant="secondary" className="font-bold text-lg">{status === 'upcoming' ? '-' : match.t2_score}</Text>
                    </div>
                  </div>
                  
                  {/* Footer with spirit and field */}
                  <div className="mt-3 pt-3 border-t border-gray-200 dark:border-slate-700 flex items-center justify-between">
                    {status === 'done' && (
                      <div className="flex gap-4">
                        <div className="text-center">
                          <Text variant="secondary" className="text-xs">Spirit</Text>
                          <Text variant="primary" className="text-xs font-bold">{match.t1_spirit ?? '-'}</Text>
                        </div>
                        <div className="text-center">
                          <Text variant="secondary" className="text-xs">Spirit</Text>
                          <Text variant="secondary" className="text-xs">{match.t2_spirit ?? '-'}</Text>
                        </div>
                      </div>
                    )}
                    {status !== 'done' && <div />}
                    <Text variant="secondary" className="text-xs">{match.field_name}</Text>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
