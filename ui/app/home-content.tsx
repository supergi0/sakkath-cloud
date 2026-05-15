'use client';

import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import Image from 'next/image';
import Link from 'next/link';
import { Calendar, ChevronDown, ChevronUp, Clock, LayoutGrid, MapPin, Target, Trophy, Users } from 'lucide-react';
import { Text } from './components/Text';
import { apiUrl } from './lib/api';
import { getTeamAbbreviation } from './lib/team-name';

function subscribeToHydration() {
  return () => {};
}

function getHydratedSnapshot() {
  return true;
}

function getServerHydratedSnapshot() {
  return false;
}

interface TeamStanding {
  id: number;
  name: string;
  abbreviation?: string | null;
  location: string;
  init_rank: number;
  wins: number;
  losses: number;
  draws: number;
  median_buchholz: number;
  buchholz: number;
  diff: number;
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

type StandingsSortField = 'rank' | 'wins' | 'losses' | 'diff' | 'spirit';

interface DashboardPreferences {
  division: 'open' | 'women';
}

const DASHBOARD_PREFS_KEY = 'sakkath:dashboard:preferences';

function readDashboardPreferences(): DashboardPreferences {
  if (typeof window === 'undefined') {
    return { division: 'open' };
  }

  const savedPreferences = localStorage.getItem(DASHBOARD_PREFS_KEY);
  if (!savedPreferences) {
    return { division: 'open' };
  }

  try {
    const parsed: DashboardPreferences = JSON.parse(savedPreferences);
    if (parsed.division === 'open' || parsed.division === 'women') {
      return parsed;
    }
  } catch {
  }

  return { division: 'open' };
}

const STANDINGS_COLUMN_CLASSES = {
  rank: 'w-10 min-w-[40px]',
  team: 'w-[172px] min-w-[172px]',
  played: 'w-[44px] min-w-[44px]',
  wins: 'w-[46px] min-w-[46px]',
  losses: 'w-[46px] min-w-[46px]',
  draws: 'w-[46px] min-w-[46px]',
  mbh: 'w-[56px] min-w-[56px]',
  nbh: 'w-[56px] min-w-[56px]',
  diff: 'w-[60px] min-w-[60px]',
  spirit: 'w-[72px] min-w-[72px]',
} as const;

function StandingsSortIndicator({ active, direction }: { active: boolean; direction: 'asc' | 'desc' }) {
  const activeClass = 'text-blue-700 dark:text-cyan-300';
  const inactiveClass = 'text-gray-400 dark:text-slate-500';

  return (
    <span className="ml-1 flex flex-col leading-none">
      <ChevronUp className={`h-3 w-3 ${active && direction === 'asc' ? activeClass : inactiveClass}`} />
      <ChevronDown className={`-mt-1 h-3 w-3 ${active && direction === 'desc' ? activeClass : inactiveClass}`} />
    </span>
  );
}

export function HomeContent() {
  const mounted = useSyncExternalStore(
    subscribeToHydration,
    getHydratedSnapshot,
    getServerHydratedSnapshot,
  );
  const [division, setDivision] = useState<'open' | 'women'>(() => readDashboardPreferences().division);
  const [standingsSortField, setStandingsSortField] = useState<StandingsSortField>('rank');
  const [standingsSortDir, setStandingsSortDir] = useState<'asc' | 'desc'>('asc');
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
    const preferences: DashboardPreferences = { division };
    localStorage.setItem(DASHBOARD_PREFS_KEY, JSON.stringify(preferences));
  }, [division]);

  const sortStandings = (standings: Array<TeamStanding & { rank: number }>) => {
    return [...standings].sort((a, b) => {
      let diff = 0;

      switch (standingsSortField) {
        case 'rank':
          diff = a.rank - b.rank;
          break;
        case 'wins':
          diff = a.wins - b.wins;
          break;
        case 'losses':
          diff = a.losses - b.losses;
          break;
        case 'diff':
          diff = a.diff - b.diff;
          break;
        case 'spirit':
          diff = a.spirit_avg - b.spirit_avg;
          break;
      }

      return standingsSortDir === 'asc' ? diff : -diff;
    });
  };

  const standingsWithRank = (division === 'open' ? openStandings : womenStandings).map((team, index) => ({
    ...team,
    rank: index + 1,
  }));
  const currentStandings = sortStandings(standingsWithRank);

  const handleSort = (field: StandingsSortField) => {
    if (standingsSortField === field) {
      setStandingsSortDir((current) => current === 'asc' ? 'desc' : 'asc');
    } else {
      setStandingsSortField(field);
      setStandingsSortDir(field === 'rank' ? 'asc' : 'desc');
    }
  };

  const getDisplayTeamName = (team: TeamStanding) => {
    if (team.name.length <= 14) {
      return team.name;
    }

    return getTeamAbbreviation(team.name, team.abbreviation, 16);
  };

  const renderSortButton = (
    field: StandingsSortField,
    label: string,
    align: 'left' | 'center' = 'left'
  ) => {
    const active = standingsSortField === field;

    return (
      <button
        className={`inline-flex items-center whitespace-nowrap font-medium ${align === 'center' ? 'justify-center' : ''} ${active ? 'text-blue-700 dark:text-cyan-300' : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200'}`}
        onClick={() => handleSort(field)}
      >
        <span>{label}</span>
        <StandingsSortIndicator active={active} direction={standingsSortDir} />
      </button>
    );
  };

  if (!mounted) return null;

  return (
    <div className="min-h-screen px-4 py-4 md:px-0">
      <div className="mx-auto max-w-7xl">
        <div className="mb-5 rounded-sm bg-white p-6 dark:bg-slate-900">
          <div className="md:grid md:grid-cols-[minmax(360px,1.35fr)_minmax(320px,1fr)] md:items-center md:gap-8">
            <div className="mb-6 md:hidden">
              <Link href="/" className="flex w-full items-center justify-center p-2">
                <div className="relative h-[148px] w-full max-w-[420px] transition-opacity hover:opacity-80">
                  {mounted && (
                    <Image src="/sakkath.png" alt="Sakkath Ultimate Open" fill className="object-contain" priority />
                  )}
                </div>
              </Link>
            </div>

            <div className="hidden min-h-[230px] items-center justify-center p-2 md:flex">
              <Link href="/" className="relative h-[150px] w-full transition-opacity hover:opacity-80">
                {mounted && (
                  <Image src="/sakkath.png" alt="Sakkath Ultimate Open" fill className="object-contain" priority />
                )}
              </Link>
            </div>

            <div className="min-w-0 text-center md:flex md:flex-col md:justify-center md:text-left">
              <Link href="/" className="inline-block transition-colors hover:text-gray-700 dark:hover:text-gray-200 md:self-start">
                <Text as="h1" variant="primary" className="mb-2 text-center text-2xl font-semibold md:text-left md:text-3xl">
                  Sakkath Ultimate Open 2026
                </Text>
              </Link>
              <div className="flex flex-wrap justify-center gap-x-4 gap-y-2 text-sm md:justify-start">
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

              <div className="mt-6 flex flex-wrap justify-center gap-3 border-t border-gray-200 pt-6 dark:border-slate-700 md:justify-start">
                <div className="hidden w-full flex-wrap justify-center gap-3 sm:flex md:justify-start">
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
          </div>
        </div>

        <div className="rounded-sm bg-white px-4 pb-4 pt-0 dark:bg-slate-900 sm:px-6 sm:pb-6 sm:pt-0">
          <div className="hidden flex-col items-center gap-3 pb-4 pt-3 sm:flex">
            <Text as="h2" variant="primary" className="text-center text-xl">
              Standings
            </Text>
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

          <div className="sticky top-14 z-20 -mx-4 mb-3 border-b border-gray-200 bg-white px-4 pb-3 pt-2 dark:border-slate-700 dark:bg-slate-900 sm:hidden">
            <Text as="h2" variant="primary" className="text-center text-xl">
              Standings
            </Text>
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
              <div className="flex w-full min-w-[640px] text-sm font-medium text-gray-500 dark:text-gray-400">
                <div className={`${STANDINGS_COLUMN_CLASSES.rank} shrink-0 px-1 pb-1 text-left`}>
                  {renderSortButton('rank', '#')}
                </div>
                <div className={`${STANDINGS_COLUMN_CLASSES.team} shrink-0 px-1.5 pb-1 text-left`}>
                  <span className="inline-flex items-center whitespace-nowrap">Team</span>
                </div>
                <div className={`${STANDINGS_COLUMN_CLASSES.played} shrink-0 px-1 pb-1 text-center`}>
                  <span className="inline-flex items-center justify-center whitespace-nowrap">P</span>
                </div>
                <div className={`${STANDINGS_COLUMN_CLASSES.wins} shrink-0 px-1 pb-1 text-center`}>
                  {renderSortButton('wins', 'W', 'center')}
                </div>
                <div className={`${STANDINGS_COLUMN_CLASSES.losses} shrink-0 px-1 pb-1 text-center`}>
                  {renderSortButton('losses', 'L', 'center')}
                </div>
                <div className={`${STANDINGS_COLUMN_CLASSES.draws} shrink-0 px-1 pb-1 text-center`}>
                  <span className="inline-flex items-center justify-center whitespace-nowrap">D</span>
                </div>
                <div className={`${STANDINGS_COLUMN_CLASSES.mbh} shrink-0 px-1 pb-1 text-center`}>
                  <span className="inline-flex items-center justify-center whitespace-nowrap">MOS</span>
                </div>
                <div className={`${STANDINGS_COLUMN_CLASSES.nbh} shrink-0 px-1 pb-1 text-center`}>
                  <span className="inline-flex items-center justify-center whitespace-nowrap">NOS</span>
                </div>
                <div className={`${STANDINGS_COLUMN_CLASSES.diff} shrink-0 px-1 pb-1 text-center`}>
                  {renderSortButton('diff', 'Diff', 'center')}
                </div>
                <div className={`${STANDINGS_COLUMN_CLASSES.spirit} shrink-0 px-1 pb-1 text-center`}>
                  {renderSortButton('spirit', 'Spirit', 'center')}
                </div>
              </div>
            </div>
          </div>

          <div className="overflow-x-auto" ref={tableScrollRef} onScroll={handleTableScroll}>
            <table className="w-full min-w-[640px] table-fixed text-sm">
              <colgroup>
                <col className={STANDINGS_COLUMN_CLASSES.rank} />
                <col className={STANDINGS_COLUMN_CLASSES.team} />
                <col className={STANDINGS_COLUMN_CLASSES.played} />
                <col className={STANDINGS_COLUMN_CLASSES.wins} />
                <col className={STANDINGS_COLUMN_CLASSES.losses} />
                <col className={STANDINGS_COLUMN_CLASSES.draws} />
                <col className={STANDINGS_COLUMN_CLASSES.mbh} />
                <col className={STANDINGS_COLUMN_CLASSES.nbh} />
                <col className={STANDINGS_COLUMN_CLASSES.diff} />
                <col className={STANDINGS_COLUMN_CLASSES.spirit} />
              </colgroup>
              <thead className="hidden bg-white dark:bg-slate-900 sm:table-header-group">
                <tr className="border-b border-gray-200 dark:border-slate-700">
                  <th className={`${STANDINGS_COLUMN_CLASSES.rank} px-1 py-3 text-left font-medium text-gray-500 dark:text-gray-400 sm:px-2`}>
                    {renderSortButton('rank', '#')}
                  </th>
                  <th className={`${STANDINGS_COLUMN_CLASSES.team} px-1.5 py-3 text-left font-medium text-gray-500 dark:text-gray-400 sm:px-2`}>
                    <span className="inline-flex items-center whitespace-nowrap">Team</span>
                  </th>
                  <th className={`${STANDINGS_COLUMN_CLASSES.played} px-1 py-3 text-center font-medium text-gray-500 dark:text-gray-400 sm:px-2`}>
                    <span className="inline-flex items-center justify-center whitespace-nowrap">P</span>
                  </th>
                  <th className={`${STANDINGS_COLUMN_CLASSES.wins} px-1 py-3 text-center font-medium text-gray-500 dark:text-gray-400 sm:px-2`}>
                    {renderSortButton('wins', 'W', 'center')}
                  </th>
                  <th className={`${STANDINGS_COLUMN_CLASSES.losses} px-1 py-3 text-center font-medium text-gray-500 dark:text-gray-400 sm:px-2`}>
                    {renderSortButton('losses', 'L', 'center')}
                  </th>
                  <th className={`${STANDINGS_COLUMN_CLASSES.draws} px-1 py-3 text-center font-medium text-gray-500 dark:text-gray-400 sm:px-2`}>
                    <span className="inline-flex items-center justify-center whitespace-nowrap">D</span>
                  </th>
                  <th className={`${STANDINGS_COLUMN_CLASSES.mbh} px-1 py-3 text-center font-medium text-gray-500 dark:text-gray-400 sm:px-2`}>
                    <span className="inline-flex items-center justify-center whitespace-nowrap">MBH</span>
                  </th>
                  <th className={`${STANDINGS_COLUMN_CLASSES.nbh} px-1 py-3 text-center font-medium text-gray-500 dark:text-gray-400 sm:px-2`}>
                    <span className="inline-flex items-center justify-center whitespace-nowrap">NBH</span>
                  </th>
                  <th className={`${STANDINGS_COLUMN_CLASSES.diff} px-1 py-3 text-center font-medium text-gray-500 dark:text-gray-400 sm:px-2`}>
                    {renderSortButton('diff', 'Diff', 'center')}
                  </th>
                  <th className={`${STANDINGS_COLUMN_CLASSES.spirit} px-1 py-3 text-center font-medium text-gray-500 dark:text-gray-400 sm:px-2`}>
                    {renderSortButton('spirit', 'Spirit', 'center')}
                  </th>
                </tr>
              </thead>
              <tbody>
                {!loading && currentStandings.map((team) => {
                  const diff = team.diff;
                  const initial = team.name.charAt(0).toUpperCase();
                  const displayName = getDisplayTeamName(team);

                  return (
                    <tr key={team.id} className="border-b border-gray-200 hover:opacity-80 dark:border-slate-700">
                      <td className={`${STANDINGS_COLUMN_CLASSES.rank} px-1 py-3 sm:px-2`}>
                        <Text variant="primary" className="font-medium">{team.rank}</Text>
                      </td>
                      <td className={`${STANDINGS_COLUMN_CLASSES.team} px-1.5 py-3 overflow-hidden sm:px-2`}>
                        <Link href={`/teams?team_id=${team.id}`} className="flex items-center gap-1.5 hover:underline sm:gap-2" title={team.name}>
                          <div className="relative flex h-7 w-7 flex-shrink-0 items-center justify-center overflow-hidden rounded-full bg-blue-900 text-xs font-bold text-white sm:h-6 sm:w-6">
                            {team.small_logo ? (
                              <Image
                                src={team.small_logo}
                                alt={team.name}
                                fill
                                unoptimized
                                sizes="28px"
                                className="object-cover"
                              />
                            ) : (
                              initial
                            )}
                          </div>
                          <Text variant="primary" className="block min-w-0 truncate font-medium whitespace-nowrap">{displayName}</Text>
                        </Link>
                      </td>
                      <td className={`${STANDINGS_COLUMN_CLASSES.played} px-1 py-3 text-center sm:px-2`}>
                        <Text variant="secondary">{team.wins + team.losses + team.draws}</Text>
                      </td>
                      <td className={`${STANDINGS_COLUMN_CLASSES.wins} px-1 py-3 text-center text-green-500 sm:px-2`}>{team.wins}</td>
                      <td className={`${STANDINGS_COLUMN_CLASSES.losses} px-1 py-3 text-center text-red-500 sm:px-2`}>{team.losses}</td>
                      <td className={`${STANDINGS_COLUMN_CLASSES.draws} px-1 py-3 text-center sm:px-2`}>
                        <Text variant="primary">{team.draws}</Text>
                      </td>
                      <td className={`${STANDINGS_COLUMN_CLASSES.mbh} px-1 py-3 text-center sm:px-2`}>
                        <Text variant="primary">{team.median_buchholz}</Text>
                      </td>
                      <td className={`${STANDINGS_COLUMN_CLASSES.nbh} px-1 py-3 text-center sm:px-2`}>
                        <Text variant="primary">{team.buchholz}</Text>
                      </td>
                      <td className={`${STANDINGS_COLUMN_CLASSES.diff} px-1 py-3 text-center font-medium sm:px-2 ${diff > 0 ? 'text-green-500' : diff < 0 ? 'text-red-500' : 'text-gray-900 dark:text-white'}`}>
                        {diff > 0 ? '+' : ''}{diff}
                      </td>
                      <td className={`${STANDINGS_COLUMN_CLASSES.spirit} px-1 py-3 text-center sm:px-2`}>
                        <Text variant="primary">{team.spirit_avg.toFixed(1)}</Text>
                      </td>
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