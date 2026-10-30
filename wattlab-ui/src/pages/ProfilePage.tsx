import { useEffect, useMemo, useState } from 'react';
import { UnauthorizedError, type ApiClient } from '../api/client';
import type { ActivityEntry, Member } from '../api/types';
import { useAppearance } from '../i18n/appearance';
import type { MessageKey } from '../i18n/messages';
import {
  activityLevel,
  formatHours,
  groupActivity,
  heatmapWeeks,
  longestStreak,
  monthLabels,
  qualifyingStreakDays,
  recentStreak,
  yearHeatmapRange,
  type DayActivity,
} from '../model/activity';

const WEEKDAYS: MessageKey[] = [
  'weekdaySun',
  'weekdayMon',
  'weekdayTue',
  'weekdayWed',
  'weekdayThu',
  'weekdayFri',
  'weekdaySat',
];

interface Props {
  client: ApiClient;
  user: Member;
  onUnauthorized(): void;
  now?: () => Date;
}

export function ProfilePage({ client, user, onUnauthorized, now = () => new Date() }: Props) {
  const { t, locale } = useAppearance();
  const thisYear = now().getFullYear();
  const [year, setYear] = useState(thisYear);
  const range = useMemo(() => yearHeatmapRange(year), [year]);
  const [entries, setEntries] = useState<ActivityEntry[]>([]);
  const [state, setState] = useState<'loading' | 'ready' | 'failed'>('loading');
  const [selected, setSelected] = useState<DayActivity | null>(null);

  useEffect(() => {
    let active = true;
    setState('loading');
    client.activity(range.from.toISOString(), range.to.toISOString()).then(
      (activity) => {
        if (!active) return;
        setEntries(activity.entries);
        setSelected(null);
        setState('ready');
      },
      (error) => {
        if (!active) return;
        if (error instanceof UnauthorizedError) onUnauthorized();
        else setState('failed');
      },
    );
    return () => {
      active = false;
    };
  }, [client, onUnauthorized, range.from, range.to]);

  const days = useMemo(() => groupActivity(entries), [entries]);
  const qualified = useMemo(() => qualifyingStreakDays(entries), [entries]);
  const weeks = useMemo(() => heatmapWeeks(range, days), [range, days]);
  const months = useMemo(() => monthLabels(weeks, locale), [weeks, locale]);
  const last = useMemo(() => recentStreak(qualified, range, now()), [now, qualified, range]);
  const longest = useMemo(() => longestStreak(qualified, range), [qualified, range]);
  const totals = useMemo(() => {
    let sessions = 0;
    let duration = 0;
    let activeDays = 0;
    for (const day of days.values()) {
      if (!day.sessions) continue;
      sessions += day.sessions;
      duration += day.durationSeconds;
      activeDays += 1;
    }
    return { sessions, duration, activeDays };
  }, [days]);

  return (
    <>
      <section className="page-heading">
        <div>
          <p className="eyebrow">{t('profileEyebrow')}</p>
          <h1>{user.username}</h1>
          <p className="subtitle">{t('profileSubtitle')}</p>
        </div>
        <div className="year-picker" role="group" aria-label={t('activityYear')}>
          <button type="button" className="secondary" disabled={year <= thisYear - 10} onClick={() => setYear(year - 1)}>
            ‹
          </button>
          <strong>{year}</strong>
          <button type="button" className="secondary" disabled={year >= thisYear} onClick={() => setYear(year + 1)}>
            ›
          </button>
        </div>
      </section>
      <section className="list-card profile-card">
        <div className="profile-totals">
          <p>
            <strong>{totals.sessions}</strong> {t('workouts')}
          </p>
          <p>
            <strong>{formatHours(totals.duration, locale)}</strong>
          </p>
          <p>
            <strong>{totals.activeDays}</strong> {t('days')}
          </p>
          <p data-testid="current-streak">
            <strong>{last}</strong> {t('lastStreak')}
          </p>
          <p data-testid="longest-streak">
            <strong>{longest}</strong> {t('longestStreak')}
          </p>
          <span className="info-tip">
            <button type="button" className="info-icon" title={t('streakRule')} aria-label={t('streakRule')}>
              i
            </button>
          </span>
        </div>
        {state === 'failed' ? (
          <p className="access-error" role="alert">
            {t('activityFailed')}
          </p>
        ) : null}
        <div className="heatmap" aria-label={t('yearCalendar')}>
          <div className="heatmap-months">
            {months.map((month) => (
              <span key={`${month.label}-${month.index}`} style={{ gridColumn: month.index + 1 }}>
                {month.label}
              </span>
            ))}
          </div>
          <div className="heatmap-body">
            <div className="heatmap-dows">
              {WEEKDAYS.map((day) => (
                <span key={day}>{t(day)}</span>
              ))}
            </div>
            <div className="heatmap-weeks">
              {weeks.map((week) => (
                <div key={week[0].date} className="heatmap-week">
                  {week.map((day) => (
                    <button
                      key={day.date}
                      type="button"
                      className={`heat heat-${activityLevel(day.sessions)}${selected?.date === day.date ? ' selected' : ''}`}
                      title={t('daySummary', {
                        date: day.date,
                        sessions: day.sessions,
                        hours: formatHours(day.durationSeconds, locale),
                      })}
                      aria-label={t('daySummary', {
                        date: day.date,
                        sessions: day.sessions,
                        hours: formatHours(day.durationSeconds, locale),
                      })}
                      onClick={() => setSelected(day)}
                    />
                  ))}
                </div>
              ))}
            </div>
          </div>
          <div className="heatmap-legend">
            {t('less')}
            <span className="heat heat-0" />
            <span className="heat heat-1" />
            <span className="heat heat-2" />
            <span className="heat heat-3" />
            <span className="heat heat-4" />
            {t('more')}
          </div>
        </div>
        <p className="profile-day">
          {selected
            ? t('daySummary', {
                date: selected.date,
                sessions: selected.sessions,
                hours: formatHours(selected.durationSeconds, locale),
              })
            : t('pickDay')}
        </p>
        {state === 'loading' ? <p className="empty">{t('loading')}</p> : null}
      </section>
    </>
  );
}
