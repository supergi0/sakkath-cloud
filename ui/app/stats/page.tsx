'use client';

import { useEffect, useState } from "react";
import { ChevronUp, ChevronDown } from "lucide-react";
import { Text } from "../components/Text";

interface PlayerStat {
  id: number;
  name: string;
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

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:9000';

type SortField = 'name' | 'team' | 'goals' | 'assists' | 'blocks' | 'turnovers' | 'matches' | 'gpm' | 'apm' | 'bpm' | 'tpm';
type SortDir = 'asc' | 'desc';

const PAGE_SIZE = 20;

export default function Stats() {
  const [players, setPlayers] = useState<PlayerStat[]>([]);
  const [teams, setTeams] = useState<Team[]>([]);
  const [loading, setLoading] = useState(true);
  const [division, setDivision] = useState<'all' | 'open' | 'women'>('all');
  const [teamFilter, setTeamFilter] = useState<number | null>(null);
  const [sortField, setSortField] = useState<SortField>('goals');
  const [sortDir, setSortDir] = useState<SortDir>('desc');
  const [page, setPage] = useState(1);

  useEffect(() => {
    Promise.all([
      fetch(`${API_URL}/v1/player-stats`).then(r => r.json()),
      fetch(`${API_URL}/v1/teams`).then(r => r.json()),
    ]).then(([playersData, teamsData]) => {
      setPlayers(playersData);
      setTeams(teamsData);
      setLoading(false);
    }).catch(() => setLoading(false));
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
      <div className="min-h-screen">
        <div className="sticky top-16 md:top-[72px] z-20 bg-white dark:bg-slate-900 shadow-sm">
          <div className="max-w-7xl mx-auto px-4 py-6">
            <Text variant="primary" className="text-xl">Player Statistics</Text>
          </div>
        </div>
        <div className="max-w-7xl mx-auto px-4 py-8">
          <Text variant="secondary">Loading...</Text>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-100 dark:bg-slate-950">
      {/* Sticky header - no gap with navbar */}
      <div className="sticky top-16 md:top-[72px] z-20 bg-white dark:bg-slate-900 shadow-sm">
        <div className="max-w-7xl mx-auto px-4 py-4">
          <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
            <Text as="h1" variant="primary" className="text-xl">
              Player Statistics
            </Text>
            <div className="flex gap-2 w-full sm:w-auto">
              <div className="relative">
                <select
                  value={division}
                  onChange={(e) => { setDivision(e.target.value as typeof division); setTeamFilter(null); setPage(1); }}
                  className="appearance-none px-3 py-2 pr-8 text-sm rounded bg-gray-100 dark:bg-slate-800 border-0 text-gray-700 dark:text-gray-300 cursor-pointer"
                >
                  <option value="all">All Divisions</option>
                  <option value="open">Open</option>
                  <option value="women">Women</option>
                </select>
                <ChevronDown className="absolute right-2 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500 pointer-events-none" />
              </div>
              <div className="relative flex-1 sm:flex-none">
                <select
                  value={teamFilter ?? ''}
                  onChange={(e) => { setTeamFilter(e.target.value ? Number(e.target.value) : null); setPage(1); }}
                  className="appearance-none w-full px-3 py-2 pr-8 text-sm rounded bg-gray-100 dark:bg-slate-800 border-0 text-gray-700 dark:text-gray-300 cursor-pointer sm:min-w-[180px]"
                >
                  <option value="">All Teams</option>
                  {filteredTeams.map(t => (
                    <option key={t.id} value={t.id}>{t.name}</option>
                  ))}
                </select>
                <ChevronDown className="absolute right-2 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500 pointer-events-none" />
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Table section */}
      <div className="max-w-7xl mx-auto px-4 py-4">
        <div className="rounded-sm bg-white dark:bg-slate-900 overflow-hidden">
          <div className="overflow-x-auto max-h-[calc(100vh-240px)] overflow-y-auto">
            <table className="w-full text-xs sm:text-sm min-w-[800px]">
              <thead className="sticky top-0 bg-white dark:bg-slate-900 z-10">
                <tr className="border-b border-gray-200 dark:border-slate-700">
                  <SortHeader field="name" label="Player" className="text-left" />
                  <SortHeader field="team" label="Team" className="text-left" />
                  <SortHeader field="goals" label="Gls" />
                  <SortHeader field="assists" label="Ast" />
                  <SortHeader field="blocks" label="Blk" />
                  <SortHeader field="turnovers" label="TO" />
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
                      className="hover:opacity-80 border-b border-gray-200 dark:border-slate-700"
                    >
                      <td className="py-3 px-2">
                        <Text variant="primary" className="font-medium truncate max-w-[150px] block">{player.name}</Text>
                      </td>
                      <td className="py-3 px-2">
                        <Text variant="secondary" className="truncate max-w-[120px] block">{player.team_name}</Text>
                      </td>
                      <td className="py-3 px-2 text-center">
                        <Text variant="primary">{player.goals}</Text>
                      </td>
                      <td className="py-3 px-2 text-center">
                        <Text variant="primary">{player.assists}</Text>
                      </td>
                      <td className="py-3 px-2 text-center">
                        <Text variant="primary">{player.blocks}</Text>
                      </td>
                      <td className="py-3 px-2 text-center">
                        <Text variant="primary">{player.turnovers}</Text>
                      </td>
                      <td className="py-3 px-2 text-center">
                        <Text variant="secondary">{player.matches}</Text>
                      </td>
                      <td className="py-3 px-2 text-center">
                        <Text variant="secondary">{(player.goals / m).toFixed(2)}</Text>
                      </td>
                      <td className="py-3 px-2 text-center">
                        <Text variant="secondary">{(player.assists / m).toFixed(2)}</Text>
                      </td>
                      <td className="py-3 px-2 text-center">
                        <Text variant="secondary">{(player.blocks / m).toFixed(2)}</Text>
                      </td>
                      <td className="py-3 px-2 text-center">
                        <Text variant="secondary">{(player.turnovers / m).toFixed(2)}</Text>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          {totalPages > 1 && (
            <div className="flex items-center justify-center gap-2 p-4 border-t border-gray-200 dark:border-slate-700">
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
    </div>
  );
}