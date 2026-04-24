'use client';

import { useEffect, useRef, useState } from "react";
import { ChevronUp, ChevronDown } from "lucide-react";
import { Text } from "../components/Text";
import { apiUrl } from "../lib/api";

interface PlayerStat {
  id: number;
  name: string;
  common_name: string;
  team_id: number;
  team_name: string;
  division: number;
  goals: number;
  assists: number;
  blocks: number;
  turnovers: number;
  matches: number;
}

interface Team {
  id: number;
  name: string;
  division: number;
}

type SortField = 'name' | 'team' | 'goals' | 'assists' | 'blocks' | 'turnovers' | 'matches' | 'gpm' | 'apm' | 'bpm' | 'tpm';
type SortDir = 'asc' | 'desc';
interface StatsPreferences {
  division: 'all' | 'open' | 'women';
  teamFilter: number | null;
}

const STATS_PREFS_KEY = 'sakkath:stats:preferences';

const PAGE_SIZE = 32;

export default function Stats() {
  const [players, setPlayers] = useState<PlayerStat[]>([]);
  const [teams, setTeams] = useState<Team[]>([]);
  const [loading, setLoading] = useState(true);
  const [division, setDivision] = useState<'all' | 'open' | 'women'>('all');
  const [teamFilter, setTeamFilter] = useState<number | null>(null);
  const [sortField, setSortField] = useState<SortField>('goals');
  const [sortDir, setSortDir] = useState<SortDir>('desc');
  const [page, setPage] = useState(1);
  const [tableStickyTop, setTableStickyTop] = useState(112);
  const headerRef = useRef<HTMLDivElement | null>(null);
  const [showingCommonNames, setShowingCommonNames] = useState<Record<number, boolean>>({});

  const togglePlayerName = (playerId: number) => {
    setShowingCommonNames((current) => ({
      ...current,
      [playerId]: !current[playerId],
    }));
  };

  const truncateName = (name: string, maxLen: number = 18) => {
    if (!name) return '';
    if (name.length <= maxLen) return name;
    const parts = name.trim().split(/\s+/);
    if (parts.length <= 1) return name.substring(0, maxLen - 3) + '...';
    
    let current = [...parts];
    for (let i = current.length - 1; i > 0; i--) {
      current[i] = current[i][0] + '.';
      const joined = current.join(' ');
      if (joined.length <= maxLen) return joined;
    }
    
    const joined = current.join(' ');
    if (joined.length > maxLen) {
      return joined.substring(0, maxLen - 3) + '...';
    }
    return joined;
  };

  const renderPlayerName = (player: PlayerStat) => {
    const commonName = player.common_name?.trim();
    const hasAlternateName = Boolean(commonName && commonName !== player.name);
    const displayName = truncateName(player.name);

    if (!hasAlternateName) {
      return <Text variant="primary" className="font-medium block truncate w-full">{displayName}</Text>;
    }

    const showingCommonName = Boolean(showingCommonNames[player.id]);
    const displayCommon = truncateName(commonName);

    return (
      <div className="relative inline-block pointer-events-none align-middle w-full">
        <span className="relative block h-[1.5rem] w-full overflow-hidden">
          <span
            className={`block truncate font-medium text-gray-900 transition-all duration-300 dark:text-white pointer-events-none ${
              showingCommonName ? '-translate-y-full scale-95 opacity-0' : 'translate-y-0 scale-100 opacity-100'
            }`}
          >
            {displayName}
          </span>
          <span
            className={`absolute inset-0 block truncate font-medium text-blue-700 transition-all duration-300 dark:text-cyan-300 pointer-events-none ${
              showingCommonName ? 'translate-y-0 scale-100 opacity-100' : 'translate-y-full scale-95 opacity-0'
            }`}
          >
            {displayCommon}
          </span>
        </span>
      </div>
    );
  };

  useEffect(() => {
    const savedPreferences = localStorage.getItem(STATS_PREFS_KEY);
    if (savedPreferences) {
      try {
        const parsed: StatsPreferences = JSON.parse(savedPreferences);
        if (parsed.division === 'all' || parsed.division === 'open' || parsed.division === 'women') {
          setDivision(parsed.division);
        }
        if (typeof parsed.teamFilter === 'number' || parsed.teamFilter === null) {
          setTeamFilter(parsed.teamFilter);
        }
      } catch {
      }
    }

    Promise.all([
      fetch(apiUrl('/v1/player-stats')).then(r => r.json()),
      fetch(apiUrl('/v1/teams')).then(r => r.json()),
    ]).then(([playersData, teamsData]) => {
      setPlayers(playersData);
      setTeams(teamsData);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  useEffect(() => {
    const preferences: StatsPreferences = { division, teamFilter };
    localStorage.setItem(STATS_PREFS_KEY, JSON.stringify(preferences));
  }, [division, teamFilter]);

  useEffect(() => {
    const updateStickyTop = () => {
      const navbarHeight = 56;
      const headerHeight = headerRef.current?.offsetHeight ?? 0;
      setTableStickyTop(navbarHeight + headerHeight);
    };

    updateStickyTop();
    window.addEventListener('resize', updateStickyTop);

    return () => window.removeEventListener('resize', updateStickyTop);
  }, []);

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDir(sortDir === 'asc' ? 'desc' : 'asc');
    } else {
      setSortField(field);
      setSortDir('desc');
    }
    setPage(1);
  };

  const filteredPlayers = players.filter(p => {
    if (division === 'open' && p.division !== 0) return false;
    if (division === 'women' && p.division !== 1) return false;
    if (teamFilter !== null && p.team_id !== teamFilter) return false;
    return true;
  });

  const sortedPlayers = [...filteredPlayers].sort((a, b) => {
    const aMatches = a.matches || 1;
    const bMatches = b.matches || 1;
    let aVal: number | string, bVal: number | string;
    
    switch (sortField) {
      case 'name': aVal = a.name; bVal = b.name; break;
      case 'team': aVal = a.team_name; bVal = b.team_name; break;
      case 'goals': aVal = a.goals; bVal = b.goals; break;
      case 'assists': aVal = a.assists; bVal = b.assists; break;
      case 'blocks': aVal = a.blocks; bVal = b.blocks; break;
      case 'turnovers': aVal = a.turnovers; bVal = b.turnovers; break;
      case 'matches': aVal = a.matches; bVal = b.matches; break;
      case 'gpm': aVal = a.goals / aMatches; bVal = b.goals / bMatches; break;
      case 'apm': aVal = a.assists / aMatches; bVal = b.assists / bMatches; break;
      case 'bpm': aVal = a.blocks / aMatches; bVal = b.blocks / bMatches; break;
      case 'tpm': aVal = a.turnovers / aMatches; bVal = b.turnovers / bMatches; break;
      default: aVal = a.goals; bVal = b.goals;
    }
    
    if (typeof aVal === 'string') {
      return sortDir === 'asc' ? aVal.localeCompare(bVal as string) : (bVal as string).localeCompare(aVal);
    }
    return sortDir === 'asc' ? aVal - (bVal as number) : (bVal as number) - aVal;
  });

  const totalPages = Math.ceil(sortedPlayers.length / PAGE_SIZE);
  const paginatedPlayers = sortedPlayers.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);

  const SortHeader = ({ field, label, className = '' }: { field: SortField; label: string; className?: string }) => (
    <th 
      className={`py-3 px-1 md:px-2 font-medium cursor-pointer hover:text-gray-700 dark:hover:text-gray-200 whitespace-nowrap ${className} ${
        sortField === field ? 'text-gray-900 dark:text-white font-bold' : 'text-gray-500 dark:text-gray-400'
      }`}
      onClick={() => handleSort(field)}
    >
      <div className="flex items-center justify-center gap-1">
        <span className="text-xs md:text-sm">{label}</span>
        {sortField === field && (
          sortDir === 'asc' ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />
        )}
      </div>
    </th>
  );

  const filteredTeams = teams.filter(t => {
    if (division === 'open') return t.division === 0;
    if (division === 'women') return t.division === 1;
    return true;
  });

  if (loading) {
    return (
      <div className="bg-gray-100 dark:bg-slate-950">
        <div className="px-4 py-4 sm:mx-auto sm:max-w-7xl sm:px-4">
          <Text variant="primary" className="text-xl">Player Statistics</Text>
        </div>
        <div className="px-4 py-8 sm:mx-auto sm:max-w-7xl sm:px-4">
          <Text variant="secondary">Loading...</Text>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-gray-100 dark:bg-slate-950 pb-2 sm:pb-4">
      <div className="sticky top-14 z-30 bg-gray-100 dark:bg-slate-950">
        <div ref={headerRef} className="px-4 py-3 sm:mx-auto sm:max-w-7xl sm:px-4">
          <div className="flex flex-col items-start justify-between gap-3 sm:flex-row sm:items-center">
            <Text as="h1" variant="primary" className="text-xl">
              Player Statistics
            </Text>
            <div className="flex w-full gap-2 sm:w-auto">
              <div className="relative">
                <select
                  value={division}
                  onChange={(e) => { setDivision(e.target.value as typeof division); setTeamFilter(null); setPage(1); }}
                  className="appearance-none rounded bg-white px-3 py-2 pr-8 text-sm text-gray-700 shadow-sm ring-1 ring-gray-200 dark:bg-slate-900 dark:text-gray-300 dark:ring-slate-700"
                >
                  <option value="all">All Divisions</option>
                  <option value="open">Open</option>
                  <option value="women">Women</option>
                </select>
                <ChevronDown className="pointer-events-none absolute right-2 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-500" />
              </div>
              <div className="relative flex-1 sm:flex-none">
                <select
                  value={teamFilter ?? ''}
                  onChange={(e) => { setTeamFilter(e.target.value ? Number(e.target.value) : null); setPage(1); }}
                  className="appearance-none w-full rounded bg-white px-3 py-2 pr-8 text-sm text-gray-700 shadow-sm ring-1 ring-gray-200 dark:bg-slate-900 dark:text-gray-300 dark:ring-slate-700 sm:min-w-[180px]"
                >
                  <option value="">All Teams</option>
                  {filteredTeams.map(t => (
                    <option key={t.id} value={t.id}>{t.name}</option>
                  ))}
                </select>
                <ChevronDown className="pointer-events-none absolute right-2 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-500" />
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="pb-2 sm:mx-auto sm:max-w-7xl sm:px-4">
        <div className="overflow-x-auto overflow-y-auto max-h-[calc(100vh-140px)] border-y border-gray-200 bg-white dark:border-slate-700 dark:bg-slate-900 sm:rounded-sm sm:border">
          <table className="w-full min-w-[720px] text-xs sm:min-w-[800px] sm:text-sm">
            <thead className="sticky top-0 z-20 bg-white dark:bg-slate-900 shadow-[0_2px_4px_rgba(0,0,0,0.05)] dark:shadow-[0_2px_4px_rgba(0,0,0,0.2)]">
              <tr className="border-b border-gray-200 dark:border-slate-700">
                  <SortHeader field="name" label="Player" className="text-left sticky left-0 bg-white dark:bg-slate-900 z-30 w-[140px] max-w-[140px]" />
                  <SortHeader field="goals" label="Gls" />
                  <SortHeader field="assists" label="Ast" />
                  <SortHeader field="blocks" label="Blk" />
                  <SortHeader field="turnovers" label="Tvr" />
                  <SortHeader field="matches" label="M" />
                  <SortHeader field="gpm" label="G/M" />
                  <SortHeader field="apm" label="A/M" />
                  <SortHeader field="bpm" label="B/M" />
                  <SortHeader field="tpm" label="T/M" />
                </tr>
              </thead>
              <tbody>
                {paginatedPlayers.map((player) => {
                  const m = player.matches || 1;
                  return (
                    <tr 
                      key={player.id} 
                      onClick={() => togglePlayerName(player.id)}
                      className="hover:opacity-80 border-b border-gray-200 dark:border-slate-700 cursor-pointer"
                    >
                      <td className="sticky left-0 z-10 bg-white px-2 py-1.5 shadow-[2px_0_4px_-1px_rgba(0,0,0,0.08)] dark:bg-slate-900 dark:shadow-[2px_0_4px_-1px_rgba(0,0,0,0.3)] w-[140px] max-w-[140px] truncate">
                        <div className="flex flex-col justify-center pointer-events-none">
                          {renderPlayerName(player)}
                          <Text variant="secondary" className="text-[10px] leading-tight truncate">{truncateName(player.team_name, 20)}</Text>
                        </div>
                      </td>
                      <td className="px-2 py-2.5 text-center">
                        <Text variant="primary">{player.goals}</Text>
                      </td>
                      <td className="px-2 py-2.5 text-center">
                        <Text variant="primary">{player.assists}</Text>
                      </td>
                      <td className="px-2 py-2.5 text-center">
                        <Text variant="primary">{player.blocks}</Text>
                      </td>
                      <td className="px-2 py-2.5 text-center">
                        <Text variant="primary">{player.turnovers}</Text>
                      </td>
                      <td className="px-2 py-2.5 text-center">
                        <Text variant="secondary">{player.matches}</Text>
                      </td>
                      <td className="px-2 py-2.5 text-center">
                        <Text variant="secondary">{(player.goals / m).toFixed(2)}</Text>
                      </td>
                      <td className="px-2 py-2.5 text-center">
                        <Text variant="secondary">{(player.assists / m).toFixed(2)}</Text>
                      </td>
                      <td className="px-2 py-2.5 text-center">
                        <Text variant="secondary">{(player.blocks / m).toFixed(2)}</Text>
                      </td>
                      <td className="px-2 py-2.5 text-center">
                        <Text variant="secondary">{(player.turnovers / m).toFixed(2)}</Text>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
            
          {totalPages > 1 && (
            <div className="flex items-center justify-center gap-2 border-b border-gray-200 bg-white p-3 dark:bg-slate-900 dark:border-slate-700 sm:border-x sm:rounded-b-sm">
              <button
                onClick={() => setPage(p => Math.max(1, p - 1))}
                disabled={page === 1}
                className="px-3 py-1 text-sm rounded bg-gray-200 dark:bg-slate-700 disabled:opacity-50"
              >
                Prev
              </button>
              <Text variant="secondary" className="text-sm">
                {page} / {totalPages}
              </Text>
              <button
                onClick={() => setPage(p => Math.min(totalPages, p + 1))}
                disabled={page === totalPages}
                className="px-3 py-1 text-sm rounded bg-gray-200 dark:bg-slate-700 disabled:opacity-50"
              >
                Next
              </button>
            </div>
          )}
        </div>
      </div>
  );
}