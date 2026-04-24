'use client';

import { useEffect, useRef, useState } from 'react';
import Image from 'next/image';
import Link from 'next/link';
import { Calendar, Clock, LayoutGrid, MapPin, Target, Trophy, Users } from 'lucide-react';
import { Text } from './components/Text';
import { apiUrl } from './lib/api';
import { getTeamAbbreviation } from './lib/team-name';

interface TeamStanding {
  id: number;
  name: string;
  abbreviation?: string | null;
  location: string;
  init_rank: number;
  wins: number;
  losses: number;
  points_for: number;
  points_against: number;
  spirit_avg: number;
  small_logo?: string | null;
}

interface Stats {
  teams: number;
  players: number;
  points: number;
  games: number;
  fields: number;
}

type SortBy = 'game' | 'initial' | 'spirit';

interface DashboardPreferences {
  division: 'open' | 'women';
  sortBy: SortBy;
}

const DASHBOARD_PREFS_KEY = 'sakkath:dashboard:preferences';

export function HomeContent() {
  const [mounted, setMounted] = useState(false);
  const [division, setDivision] = useState<'open' | 'women'>('open');
  const [sortBy, setSortBy] = useState<SortBy>('game');
  const [openStandings, setOpenStandings] = useState<TeamStanding[]>([]);
  const [womenStandings, setWomenStandings] = useState<TeamStanding[]>([]);
  const [stats, setStats] = useState<Stats>({ teams: 0, players: 0, points: 0, games: 0, fields: 0 });
  const [loading, setLoading] = useState(true);
  
  const headerScrollRef = useRef<HTMLDivElement>(null);
  const tableScrollRef = useRef<HTMLDivElement>(null);

  const handleTableScroll = (e: React.UIEvent<HTMLDivElement>) => {
    if (headerScrollRef.current) {
      headerScrollRef.current.scrollLeft = e.currentTarget.scrollLeft;
    }
  };

  useEffect(() => {
    setMounted(true);

    const savedPreferences = localStorage.getItem(DASHBOARD_PREFS_KEY);
    if (savedPreferences) {
      try {
        const parsed: DashboardPreferences = JSON.parse(savedPreferences);
        if (parsed.division === 'open' || parsed.division === 'women') {
          setDivision(parsed.division);
        }
        if (parsed.sortBy === 'game' || parsed.sortBy === 'initial' || parsed.sortBy === 'spirit') {
          setSortBy(parsed.sortBy);
        }
      } catch {
      }
    }

    Promise.all([
      fetch(apiUrl('/v1/standings?division=0')).then((response) => response.json()),
      fetch(apiUrl('/v1/standings?division=1')).then((response) => response.json()),
      fetch(apiUrl('/v1/stats')).then((response) => response.json()),
    ]).then(([open, women, statsData]) => {
      setOpenStandings(open);
      setWomenStandings(women);
      setStats(statsData);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  useEffect(() => {
    if (!mounted) return;
    const preferences: DashboardPreferences = { division, sortBy };
    localStorage.setItem(DASHBOARD_PREFS_KEY, JSON.stringify(preferences));
  }, [mounted, division, sortBy]);

  const sortStandings = (standings: TeamStanding[]) => {
    return [...standings].sort((a, b) => {
      switch (sortBy) {
        case 'initial':
          return a.init_rank - b.init_rank;
        case 'spirit':
          return b.spirit_avg - a.spirit_avg;
        case 'game':
        default: {
          const aWinPct = a.wins / (a.wins + a.losses || 1);
          const bWinPct = b.wins / (b.wins + b.losses || 1);
          if (bWinPct !== aWinPct) return bWinPct - aWinPct;
          return (b.points_for - b.points_against) - (a.points_for - a.points_against);
        }
      }
    });
  };

  const currentStandings = sortStandings(division === 'open' ? openStandings : womenStandings);

  const getDisplayTeamName = (team: TeamStanding) => {
    if (team.name.length <= 17) {
      return team.name;
    }

    return getTeamAbbreviation(team.name, team.abbreviation, 17);
  };

  if (!mounted) return null;

  return (
    <div className="min-h-screen px-4 py-4 md:px-0">
      <div className="mx-auto max-w-7xl">
        <div className="mb-5 rounded-sm bg-white p-6 dark:bg-slate-900">
          <div className="md:grid md:grid-cols-[minmax(260px,320px)_1fr_minmax(220px,280px)] md:items-stretch md:gap-6">
            <div className="mb-6 flex w-full items-stretch justify-between gap-4 md:hidden">
              <Link href="/" className="flex min-w-0 flex-1 items-center justify-center p-2">
                <div className="relative h-[112px] w-full max-w-[260px] transition-opacity hover:opacity-80">
                  {mounted && (
                    <Image src="/sakkath.png" alt="Sakkath Ultimate Open" fill className="object-contain" priority />
                  )}
                </div>
              </Link>
              <div className="flex w-[132px] items-center justify-center rounded-sm border border-gray-200 bg-gray-100 p-4 text-center dark:border-slate-700 dark:bg-slate-800">
                <Text variant="secondary" className="text-sm font-medium leading-snug">
                  Title Sponsor
                </Text>
              </div>
            </div>

            <div className="hidden min-h-[220px] items-center justify-center p-2 md:flex">
              <Link href="/" className="relative h-[210px] w-full transition-opacity hover:opacity-80">
                {mounted && (
                  <Image src="/sakkath.png" alt="Sakkath Ultimate Open" fill className="object-contain" priority />
                )}
              </Link>
            </div>

            <div className="min-w-0 text-center">
              <Link href="/" className="inline-block transition-colors hover:text-gray-700 dark:hover:text-gray-200">
                <Text as="h1" variant="primary" className="mb-2 text-center text-2xl font-semibold">
                  Sakkath Ultimate Open 2026
                </Text>
              </Link>
              <div className="flex flex-wrap justify-center gap-x-4 gap-y-2 text-sm">
                <Text variant="secondary" className="flex items-center justify-center gap-1">
                  <Calendar className="h-4 w-4" />
                  May 22 - May 24, 2026
                </Text>
                <Text variant="secondary" className="flex items-center justify-center gap-1">
                  <MapPin className="h-4 w-4" />
                  Bangalore, India
                </Text>
                <Text variant="secondary" className="hidden sm:flex items-center justify-center gap-1">
                  <Clock className="h-4 w-4" />
                  GMT+5:30
                </Text>
              </div>

              <div className="mt-6 flex flex-wrap justify-center gap-3 border-t border-gray-200 pt-6 dark:border-slate-700">
                <div className="hidden w-full flex-wrap justify-center gap-3 sm:flex">
                  {[
                    { icon: Users, value: stats.teams.toString(), label: 'Teams' },
                    { icon: Users, value: stats.players.toString(), label: 'Players' },
                    { icon: Target, value: stats.points.toString(), label: 'Points' },
                    { icon: Trophy, value: stats.games.toString(), label: 'Games' },
                    { icon: LayoutGrid, value: stats.fields.toString(), label: 'Fields' },
                    { icon: Calendar, value: '3', label: 'Days' },
                  ].map((stat) => (
                    <div key={stat.label} className="flex items-center gap-2 rounded bg-gray-100 px-4 py-2 dark:bg-slate-800">
                      <stat.icon className="h-4 w-4 text-gray-500 dark:text-gray-400" />
                      <Text variant="primary" className="font-bold">{stat.value}</Text>
                      <Text variant="secondary" className="text-sm">{stat.label}</Text>
                    </div>
                  ))}
                </div>

                <div className="flex w-full flex-col gap-3 sm:hidden">
                  <div className="flex gap-3">
                    {[
                      { icon: Users, value: stats.teams.toString(), label: 'Teams' },
                      { icon: Users, value: stats.players.toString(), label: 'Players' },
                    ].map((stat) => (
                      <div key={stat.label} className="flex flex-1 items-center gap-2 rounded bg-gray-100 px-3 py-2 dark:bg-slate-800">
                        <stat.icon className="h-4 w-4 text-gray-500 dark:text-gray-400" />
                        <Text variant="primary" className="text-sm font-bold">{stat.value}</Text>
                        <Text variant="secondary" className="text-xs">{stat.label}</Text>
                      </div>
                    ))}
                  </div>
                  <div className="flex gap-3">
                    {[
                      { icon: Target, value: stats.points.toString(), label: 'Points' },
                      { icon: Trophy, value: stats.games.toString(), label: 'Games' },
                    ].map((stat) => (
                      <div key={stat.label} className="flex flex-1 items-center gap-2 rounded bg-gray-100 px-3 py-2 dark:bg-slate-800">
                        <stat.icon className="h-4 w-4 text-gray-500 dark:text-gray-400" />
                        <Text variant="primary" className="text-sm font-bold">{stat.value}</Text>
                        <Text variant="secondary" className="text-xs">{stat.label}</Text>
                      </div>
                    ))}
                  </div>
                  <div className="flex gap-3">
                    {[
                      { icon: LayoutGrid, value: stats.fields.toString(), label: 'Fields' },
                      { icon: Calendar, value: '3', label: 'Days' },
                    ].map((stat) => (
                      <div key={stat.label} className="flex flex-1 items-center gap-2 rounded bg-gray-100 px-3 py-2 dark:bg-slate-800">
                        <stat.icon className="h-4 w-4 text-gray-500 dark:text-gray-400" />
                        <Text variant="primary" className="text-sm font-bold">{stat.value}</Text>
                        <Text variant="secondary" className="text-xs">{stat.label}</Text>
                      </div>
                    ))}
                  </div>
                </div>
              </div>


            </div>

            <div className="hidden min-h-[220px] items-center justify-center rounded-sm border border-gray-200 bg-gray-100 p-6 text-center dark:border-slate-700 dark:bg-slate-800 md:flex">
              <Text variant="secondary" className="text-xl font-medium leading-snug">
                Title Sponsor
              </Text>
            </div>
          </div>
        </div>

        <div className="rounded-sm bg-white p-4 dark:bg-slate-900 sm:p-6">
          <div className="mb-4 hidden items-center justify-between gap-3 sm:flex">
            <div className="flex items-center gap-4">
              <Text as="h2" variant="primary" className="text-xl">
                Standings
              </Text>
              <div className="flex gap-1">
                {(['game', 'initial', 'spirit'] as SortBy[]).map((selection) => (
                  <button
                    key={selection}
                    onClick={() => setSortBy(selection)}
                    className={`rounded px-3 py-2 text-sm transition-colors ${sortBy === selection ? 'bg-blue-900 text-white' : 'bg-gray-200 text-gray-600 dark:bg-slate-700 dark:text-gray-300'}`}
                  >
                    {selection === 'game' ? 'Current' : selection === 'initial' ? 'Initial' : 'Spirit'}
                  </button>
                ))}
              </div>
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => setDivision('open')}
                className={`rounded px-4 py-2 text-sm font-medium transition-colors ${division === 'open' ? 'bg-blue-900 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300 dark:bg-slate-800 dark:text-gray-300 dark:hover:bg-slate-700'}`}
              >
                Open Division
              </button>
              <button
                onClick={() => setDivision('women')}
                className={`rounded px-4 py-2 text-sm font-medium transition-colors ${division === 'women' ? 'bg-blue-900 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300 dark:bg-slate-800 dark:text-gray-300 dark:hover:bg-slate-700'}`}
              >
                Women Division
              </button>
            </div>
          </div>

          <div className="sticky top-14 z-20 -mx-4 mb-3 border-b border-gray-200 bg-white px-4 py-3 dark:border-slate-700 dark:bg-slate-900 sm:hidden">
            <Text as="h2" variant="primary" className="text-xl">
              Standings
            </Text>
            <div className="mt-3 flex gap-2">
              {(['game', 'initial', 'spirit'] as SortBy[]).map((selection) => (
                <button
                  key={selection}
                  onClick={() => setSortBy(selection)}
                  className={`flex-1 rounded px-3 py-2 text-sm transition-colors ${sortBy === selection ? 'bg-blue-900 text-white' : 'bg-gray-200 text-gray-600 dark:bg-slate-700 dark:text-gray-300'}`}
                >
                  {selection === 'game' ? 'Current' : selection === 'initial' ? 'Initial' : 'Spirit'}
                </button>
              ))}
            </div>
            <div className="mt-2 flex gap-2">
              <button
                onClick={() => setDivision('open')}
                className={`flex-1 rounded px-3 py-2.5 text-sm font-medium transition-colors ${division === 'open' ? 'bg-blue-900 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300 dark:bg-slate-800 dark:text-gray-300 dark:hover:bg-slate-700'}`}
              >
                Open
              </button>
              <button
                onClick={() => setDivision('women')}
                className={`flex-1 rounded px-3 py-2.5 text-sm font-medium transition-colors ${division === 'women' ? 'bg-blue-900 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300 dark:bg-slate-800 dark:text-gray-300 dark:hover:bg-slate-700'}`}
              >
                Women
              </button>
            </div>
            
            {/* Synced Mobile Header */}
            <div 
              ref={headerScrollRef}
              className="mt-3 -mb-3 -mx-4 px-4 pt-2 overflow-hidden border-t border-gray-200 bg-white dark:border-slate-700 dark:bg-slate-900"
            >
              <div className="w-full min-w-[368px] flex text-sm font-medium text-gray-500 dark:text-gray-400">
                <div className="w-6 min-w-[24px] px-1 pb-1 text-left shrink-0">#</div>
                <div className="w-[160px] min-w-[160px] px-1 pr-2 pb-1 text-left shrink-0">Team</div>
                <div className="w-8 min-w-[32px] px-0.5 pb-1 text-center shrink-0">P</div>
                <div className="w-8 min-w-[32px] px-0.5 pb-1 text-center shrink-0">W</div>
                <div className="w-8 min-w-[32px] px-0.5 pb-1 text-center shrink-0">L</div>
                <div className="w-10 min-w-[40px] px-0.5 pb-1 text-center shrink-0">Diff</div>
                <div className="w-12 min-w-[48px] px-0.5 pb-1 text-center shrink-0">Spirit</div>
                <div className="w-full grow shrink-0"></div>
              </div>
            </div>
          </div>

          <div className="overflow-x-auto" ref={tableScrollRef} onScroll={handleTableScroll}>
            <table className="w-full min-w-[368px] text-sm sm:min-w-[620px]">
              <thead className="hidden bg-white dark:bg-slate-900 sm:table-header-group">
                <tr className="border-b border-gray-200 dark:border-slate-700">
                  <th className="w-6 min-w-[24px] px-1 py-3 text-left font-medium text-gray-500 dark:text-gray-400 sm:w-auto sm:min-w-0 sm:px-2">#</th>
                  <th className="w-[160px] max-w-[160px] min-w-[160px] px-1 pr-2 py-3 text-left font-medium text-gray-500 dark:text-gray-400 sm:w-auto sm:min-w-0 sm:max-w-none sm:pr-1 sm:px-2">Team</th>
                  <th className="w-8 min-w-[32px] px-0.5 py-3 text-center font-medium text-gray-500 dark:text-gray-400 sm:w-auto sm:px-2">P</th>
                  <th className="w-8 min-w-[32px] px-0.5 py-3 text-center font-medium text-gray-500 dark:text-gray-400 sm:w-auto sm:px-2">W</th>
                  <th className="w-8 min-w-[32px] px-0.5 py-3 text-center font-medium text-gray-500 dark:text-gray-400 sm:w-auto sm:px-2">L</th>
                  <th className="hidden w-9 px-1 py-3 text-center font-medium text-gray-500 dark:text-gray-400 md:table-cell md:px-2">PF</th>
                  <th className="hidden w-9 px-1 py-3 text-center font-medium text-gray-500 dark:text-gray-400 md:table-cell md:px-2">PA</th>
                  <th className="w-10 min-w-[40px] px-0.5 py-3 text-center font-medium text-gray-500 dark:text-gray-400 sm:w-auto sm:px-2">Diff</th>
                  <th className="w-12 min-w-[48px] px-0.5 py-3 text-center font-medium text-gray-500 dark:text-gray-400 sm:w-auto sm:px-2">Spirit</th>
                  <th className="w-full sm:hidden"></th>
                </tr>
              </thead>
              <tbody>
                {!loading && currentStandings.map((team, index) => {
                  const diff = team.points_for - team.points_against;
                  const initial = team.name.charAt(0).toUpperCase();
                  const displayName = getDisplayTeamName(team);

                  return (
                    <tr key={team.id} className="border-b border-gray-200 hover:opacity-80 dark:border-slate-700">
                      <td className="w-6 min-w-[24px] px-1 py-3 sm:w-auto sm:min-w-0 sm:px-2">
                        <Text variant="primary" className="font-medium">{index + 1}</Text>
                      </td>
                      <td className="w-[160px] max-w-[160px] min-w-[160px] px-1 pr-2 py-3 overflow-hidden sm:w-auto sm:max-w-none sm:min-w-0 sm:overflow-visible sm:pr-1 sm:px-2">
                        <Link href={`/teams?team_id=${team.id}`} className="flex items-center gap-1.5 hover:underline sm:gap-2" title={team.name}>
                          <div className="flex h-7 w-7 flex-shrink-0 items-center justify-center overflow-hidden rounded-full bg-blue-900 text-xs font-bold text-white sm:h-6 sm:w-6">
                            {team.small_logo ? (
                              <img src={team.small_logo} alt={team.name} className="h-full w-full object-cover" />
                            ) : (
                              initial
                            )}
                          </div>
                          <Text variant="primary" className="block min-w-0 font-medium whitespace-nowrap">{displayName}</Text>
                        </Link>
                      </td>
                      <td className="w-8 min-w-[32px] px-0.5 py-3 text-center sm:w-auto sm:min-w-0 sm:px-2">
                        <Text variant="secondary">{team.wins + team.losses}</Text>
                      </td>
                      <td className="w-8 min-w-[32px] px-0.5 py-3 text-center text-green-500 sm:w-auto sm:min-w-0 sm:px-2">{team.wins}</td>
                      <td className="w-8 min-w-[32px] px-0.5 py-3 text-center text-red-500 sm:w-auto sm:min-w-0 sm:px-2">{team.losses}</td>
                      <td className="hidden px-1 py-3 text-center md:table-cell md:px-2">
                        <Text variant="primary">{team.points_for}</Text>
                      </td>
                      <td className="hidden px-1 py-3 text-center md:table-cell md:px-2">
                        <Text variant="primary">{team.points_against}</Text>
                      </td>
                      <td className={`w-10 min-w-[40px] px-0.5 py-3 text-center font-medium sm:w-auto sm:min-w-0 sm:px-2 ${diff > 0 ? 'text-green-500' : diff < 0 ? 'text-red-500' : 'text-gray-900 dark:text-white'}`}>
                        {diff > 0 ? '+' : ''}{diff}
                      </td>
                      <td className="w-12 min-w-[48px] px-0.5 py-3 text-center sm:w-auto sm:min-w-0 sm:px-2">
                        <Text variant="primary">{team.spirit_avg.toFixed(1)}</Text>
                      </td>
                      <td className="w-full sm:hidden"></td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  );
}