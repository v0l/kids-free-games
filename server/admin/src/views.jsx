import { useState, useEffect, useRef } from 'preact/hooks';
import { api, get } from './api.js';
import { act, openDialog, confirmDialog } from './overlay.jsx';
import { Avatar, PlayerForm, LinkCode, usePromise, Loaded } from './parts.jsx';
import { GAMES, gameName, slotName, duration, number, date, ago } from './format.js';

const go = (hash) => { location.hash = hash; };

export function Overview() {
  const [state] = usePromise(() => get('/stats'), []);
  return (
    <Loaded state={state}>
      {(s) => {
        const max = Math.max(1, ...s.by_day.map((d) => d.seconds));
        const top = [...s.games].sort((a, b) => b.seconds - a.seconds);
        return (
          <>
            <div class="head"><div><h1>Overview</h1><p>Everything the games have synced to this server.</p></div></div>
            <div class="tiles">
              <div class="tile"><b>{number(s.players)}</b><span>players</span></div>
              <div class="tile"><b>{number(s.active_day)}</b><span>seen in the last day</span></div>
              <div class="tile"><b>{number(s.active_week)}</b><span>seen in the last week</span></div>
              <div class="tile"><b>{duration(s.seconds)}</b><span>played over {number(s.sessions)} sessions</span></div>
              <div class="tile"><b>{number(s.scores)}</b><span>high scores</span></div>
              <div class="tile"><b>{number(s.saves)}</b><span>cloud saves</span></div>
            </div>
            <div class="panel">
              <header><h2>Play time, last 14 days</h2></header>
              <div class="bars" role="img" aria-label="Play time per day">
                {s.by_day.map((d) => (
                  <div key={d.day} title={`${d.day}: ${duration(d.seconds)}`}>
                    <i style={`height:${Math.max(3, (d.seconds / max) * 100)}%`} />
                    <span>{+d.day.slice(8)}</span>
                  </div>
                ))}
              </div>
            </div>
            <div class="panel">
              <header><h2>Games</h2></header>
              <div class="tbl">
                <table>
                  <thead><tr><th>Game</th><th class="num">Play time</th><th class="num">Sessions</th><th class="num">Players</th><th class="num">Scored</th></tr></thead>
                  <tbody>
                    {top.map((g) => (
                      <tr key={g.id} class="link" onClick={() => go(`#/games/${g.id}`)}>
                        <td><b>{gameName(g.id)}</b></td>
                        <td class="num">{duration(g.seconds)}</td>
                        <td class="num">{number(g.sessions)}</td>
                        <td class="num">{number(g.players)}</td>
                        <td class="num">{number(g.scored_players)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          </>
        );
      }}
    </Loaded>
  );
}

const PAGE = 25;

export function Players() {
  const [q, setQ] = useState('');
  const [query, setQuery] = useState('');
  const [sort, setSort] = useState('recent');
  const [offset, setOffset] = useState(0);
  const timer = useRef();
  useEffect(() => {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => { setQuery(q); setOffset(0); }, 250);
    return () => clearTimeout(timer.current);
  }, [q]);
  const [state] = usePromise(() => {
    const qs = new URLSearchParams({ limit: PAGE, offset, sort });
    if (query) qs.set('q', query);
    return get(`/players?${qs}`);
  }, [query, sort, offset]);
  const r = state.data;

  const addPlayer = async () => {
    let created;
    const ok = await openDialog({
      title: 'Add player',
      confirm: 'Add player',
      body: (p) => <PlayerForm state={p.state} />,
      onOk: async (st) => { created = await act(() => api('POST', '/players', st.read()), 'Player added'); },
    });
    if (ok && created) {
      await openDialog({ title: 'Link code', confirm: '', cancel: 'Done', body: () => <LinkCode link={created.link} name={created.player.name} /> });
      go(`#/players/${created.player.id}`);
    }
  };

  const to = r ? r.offset + r.players.length : 0;
  return (
    <>
      <div class="head">
        <div><h1>Players</h1><p>{r ? `${number(r.total)} ${r.total === 1 ? 'player' : 'players'}${query ? ' match' : ''}` : ''}</p></div>
        <button class="btn" onClick={addPlayer}>Add player</button>
      </div>
      <div class="panel">
        <header>
          <input class="search" type="text" placeholder="Search by name or id" aria-label="Search players" value={q} onInput={(e) => setQ(e.target.value)} />
          <select aria-label="Sort players" style="width:auto" value={sort} onChange={(e) => { setSort(e.target.value); setOffset(0); }}>
            <option value="recent">Last seen</option>
            <option value="playtime">Most played</option>
            <option value="created">Newest</option>
            <option value="name">Name</option>
          </select>
        </header>
        <Loaded state={state}>
          {(d) => (
            <>
              <div class="tbl">
                {d.players.length ? (
                  <table>
                    <thead><tr><th>Player</th><th>Last seen</th><th class="num">Play time</th><th class="num">Scores</th><th class="num">Saves</th></tr></thead>
                    <tbody>
                      {d.players.map((p) => (
                        <tr key={p.id} class="link" onClick={() => go(`#/players/${p.id}`)}>
                          <td><div class="who"><Avatar player={p} /><div>{p.name}<div class="id">{p.id.slice(0, 8)}</div></div></div></td>
                          <td>{ago(p.last_seen)}</td>
                          <td class="num">{duration(p.seconds)}</td>
                          <td class="num">{p.scores}</td>
                          <td class="num">{p.saves}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                ) : (
                  <div class="empty">{query ? 'No player matches that search.' : 'No players yet. Add one, or wait for the app to register a child.'}</div>
                )}
              </div>
              {d.total > d.limit && (
                <div class="pager">
                  <span>{d.offset + 1}-{to} of {number(d.total)}</span>
                  <button class="btn small quiet" disabled={!d.offset} onClick={() => setOffset(Math.max(0, offset - PAGE))}>Previous</button>
                  <button class="btn small quiet" disabled={to >= d.total} onClick={() => setOffset(offset + PAGE)}>Next</button>
                </div>
              )}
            </>
          )}
        </Loaded>
      </div>
    </>
  );
}

function ScoreForm({ game, current, state }) {
  const [v, setV] = useState(current || '');
  state.read = () => parseInt(v, 10);
  return (
    <div class="field">
      <label for="sv">{gameName(game)} score</label>
      <input id="sv" type="number" min="1" step="1" value={v} onInput={(e) => setV(e.target.value)} />
      <span class="mute">{game === 'memory' ? 'Moves: lower is better.' : 'Higher is better.'}</span>
    </div>
  );
}

function SaveForm({ slot, current, state }) {
  const [v, setV] = useState(current ? current.data : '');
  state.read = () => v;
  return (
    <div class="field">
      <label for="sd">{slotName(slot)} save{current ? `, revision ${current.rev}` : ' (new)'}</label>
      <textarea id="sd" spellcheck={false} value={v} onInput={(e) => setV(e.target.value)} />
      <span class="mute">Plain text exactly as the app stores it. Up to 64 KiB.</span>
    </div>
  );
}

export function Player({ id }) {
  const [state, reload] = usePromise(() => get(`/players/${id}`), [id]);
  const [scoreGame, setScoreGame] = useState(GAMES[0]);
  const [slot, setSlot] = useState('profile');

  return (
    <Loaded state={state}>
      {(r) => {
        const p = r.player;
        const edit = async () => {
          if (await openDialog({
            title: 'Edit player',
            body: ({ state: st }) => <PlayerForm player={p} state={st} />,
            onOk: (st) => act(() => api('PATCH', `/players/${id}`, st.read()), 'Saved'),
          })) reload();
        };
        const makeLink = async () => {
          const link = await act(() => api('POST', `/players/${id}/link`));
          openDialog({ title: 'Link code', confirm: '', cancel: 'Done', body: () => <LinkCode link={link} name={p.name} /> });
        };
        const remove = async () => {
          const ok = await confirmDialog(`Delete ${p.name}?`, `This erases ${p.name} with every save, score and play session. It cannot be undone.`, 'Delete player');
          if (!ok) return;
          await act(() => api('DELETE', `/players/${id}`), 'Player deleted');
          go('#/players');
        };
        const editScore = async (game, current) => {
          if (await openDialog({
            title: 'Set score',
            body: ({ state: st }) => <ScoreForm game={game} current={current} state={st} />,
            onOk: (st) => act(() => api('PUT', `/scores/${game}/${id}`, { score: st.read() }), 'Score saved'),
          })) reload();
        };
        const removeScore = async (game) => {
          if (!(await confirmDialog('Remove score?', `Remove ${p.name} from the ${gameName(game)} board?`, 'Remove'))) return;
          await act(() => api('DELETE', `/scores/${game}/${id}`), 'Score removed');
          reload();
        };
        const openSave = async (name) => {
          let current = null;
          try { current = await get(`/players/${id}/saves/${name}`); } catch (e) { if (e.status !== 404) return; }
          if (await openDialog({
            title: current ? 'Edit save' : 'Create save',
            wide: true,
            body: ({ state: st }) => <SaveForm slot={name} current={current} state={st} />,
            onOk: (st) => act(() => api('PUT', `/players/${id}/saves/${name}`, { data: st.read() }), 'Save written'),
          })) reload();
        };
        const removeSave = async (name) => {
          if (!(await confirmDialog('Delete save?', `Delete the ${slotName(name).toLowerCase()} save for ${p.name}? The app will start that slot fresh.`, 'Delete save'))) return;
          await act(() => api('DELETE', `/players/${id}/saves/${name}`), 'Save deleted');
          reload();
        };
        const totalTime = r.playtime.reduce((a, x) => a + x.seconds, 0);
        return (
          <>
            <a class="crumb" href="#/players">All players</a>
            <div class="head" style="margin-top:12px">
              <div class="row" style="gap:16px">
                <Avatar player={p} big />
                <div>
                  <h1>{p.name}</h1>
                  <p>Joined {date(p.created_at)}, seen {ago(p.last_seen)}<br /><span class="id">{p.id}</span></p>
                </div>
              </div>
              <div class="row">
                <button class="btn quiet" onClick={edit}>Edit</button>
                <button class="btn quiet" onClick={makeLink}>Make link code</button>
                <button class="btn danger" onClick={remove}>Delete player</button>
              </div>
            </div>
            <div class="cols">
              <div class="panel">
                <header><h2>High scores</h2></header>
                {r.scores.length ? (
                  <table>
                    <thead><tr><th>Game</th><th class="num">Best</th><th class="num">Plays</th><th /></tr></thead>
                    <tbody>
                      {r.scores.map((s) => (
                        <tr key={s.game}>
                          <td><a href={`#/games/${s.game}`}>{gameName(s.game)}</a></td>
                          <td class="num">{number(s.score)}</td>
                          <td class="num">{s.plays}</td>
                          <td class="num">
                            <button class="btn small quiet" onClick={() => editScore(s.game, s.score)}>Edit</button>{' '}
                            <button class="btn small danger" onClick={() => removeScore(s.game)}>Remove</button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                ) : <div class="empty">No scores yet.</div>}
                <div class="row" style="margin-top:12px">
                  <select style="width:auto" value={scoreGame} onChange={(e) => setScoreGame(e.target.value)}>
                    {GAMES.map((g) => <option key={g} value={g}>{gameName(g)}</option>)}
                  </select>
                  <button class="btn small quiet" onClick={() => editScore(scoreGame, (r.scores.find((x) => x.game === scoreGame) || {}).score)}>Set score</button>
                </div>
              </div>
              <div class="panel">
                <header><h2>Play time</h2><span class="mute">{r.playtime.length ? duration(totalTime) : ''}</span></header>
                {r.playtime.length ? (
                  <table><tbody>
                    {r.playtime.map((t) => (
                      <tr key={t.game}><td>{gameName(t.game)}</td><td class="num">{duration(t.seconds)}</td><td class="num mute">{t.sessions} {t.sessions === 1 ? 'session' : 'sessions'}</td></tr>
                    ))}
                  </tbody></table>
                ) : <div class="empty">Nothing played yet.</div>}
              </div>
            </div>
            <div class="panel">
              <header>
                <h2>Cloud saves</h2>
                <div class="row">
                  <select style="width:auto" value={slot} onChange={(e) => setSlot(e.target.value)}>
                    {['profile', ...GAMES].map((g) => <option key={g} value={g}>{slotName(g)}</option>)}
                  </select>
                  <button class="btn small quiet" onClick={() => openSave(slot)}>Open slot</button>
                </div>
              </header>
              {r.saves.length ? (
                <table>
                  <thead><tr><th>Slot</th><th class="num">Revision</th><th class="num">Size</th><th>Updated</th><th /></tr></thead>
                  <tbody>
                    {r.saves.map((s) => (
                      <tr key={s.slot}>
                        <td>{slotName(s.slot)}</td><td class="num">{s.rev}</td><td class="num">{number(s.size)} B</td><td>{ago(s.updated_at)}</td>
                        <td class="num">
                          <button class="btn small quiet" onClick={() => openSave(s.slot)}>View</button>{' '}
                          <button class="btn small danger" onClick={() => removeSave(s.slot)}>Delete</button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ) : <div class="empty">No saves yet.</div>}
            </div>
            <div class="panel">
              <header><h2>Signed-in devices</h2></header>
              {r.devices.length ? (
                <table><tbody>
                  {r.devices.map((d, i) => (
                    <tr key={i}><td>{d.label || 'Unnamed device'}</td><td class="mute">added {date(d.created_at)}</td><td class="mute">used {ago(d.last_used)}</td></tr>
                  ))}
                </tbody></table>
              ) : <div class="empty">No devices.</div>}
            </div>
          </>
        );
      }}
    </Loaded>
  );
}

export function Game({ game }) {
  const known = GAMES.includes(game);
  const [state, reload] = usePromise(() => (known ? get(`/scores/${game}?limit=200`) : Promise.resolve(null)), [game]);
  if (!known) return <div class="empty">No such game.</div>;
  return (
    <Loaded state={state}>
      {(r) => {
        const remove = async (e) => {
          if (!(await confirmDialog('Remove score?', `Remove ${e.name} from the ${gameName(game)} board?`, 'Remove'))) return;
          await act(() => api('DELETE', `/scores/${game}/${e.player_id}`), 'Score removed');
          reload();
        };
        return (
          <>
            <div class="head"><div><h1>{gameName(game)}</h1><p>{number(r.total)} {r.total === 1 ? 'score' : 'scores'}. {r.better === 'lower' ? 'Lower is better.' : 'Higher is better.'}</p></div></div>
            <div class="panel tbl">
              {r.entries.length ? (
                <table>
                  <thead><tr><th class="num" style="width:60px">Rank</th><th>Player</th><th class="num">Score</th><th class="num">Plays</th><th>Set</th><th /></tr></thead>
                  <tbody>
                    {r.entries.map((e) => (
                      <tr key={e.player_id}>
                        <td class="num"><b>{e.rank}</b></td>
                        <td><a class="who" style="text-decoration:none" href={`#/players/${e.player_id}`}><Avatar player={e} />{e.name}</a></td>
                        <td class="num">{number(e.score)}</td><td class="num">{e.plays}</td><td>{ago(e.achieved_at)}</td>
                        <td class="num"><button class="btn small danger" onClick={() => remove(e)}>Remove</button></td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ) : <div class="empty">Nobody has a score here yet.</div>}
            </div>
          </>
        );
      }}
    </Loaded>
  );
}
