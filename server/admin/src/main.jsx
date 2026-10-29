import { render } from 'preact';
import { useState, useEffect } from 'preact/hooks';
import './style.css';
import { get, session } from './api.js';
import { Overlay } from './overlay.jsx';
import { Overview, Players, Player, Game } from './views.jsx';
import { GAMES, gameName, parseRoute, setGames } from './format.js';

function Login({ onSignedIn }) {
  const [key, setKey] = useState('');
  const [error, setError] = useState('');
  const submit = async (e) => {
    e.preventDefault();
    session.key = key.trim();
    try {
      await get('/stats');
      session.set(session.key);
      onSignedIn();
    } catch (err) {
      session.key = '';
      setError(
        err.status === 404 ? 'The server has no admin key set. Run: wrangler secret put ADMIN_KEY'
          : err.status === 401 ? 'That key is not right.' : err.message,
      );
    }
  };
  return (
    <div class="login">
      <form onSubmit={submit}>
        <div class="top">Kids Games admin</div>
        <div class="body">
          <div class="field">
            <label for="k">Admin key</label>
            <input id="k" type="password" autocomplete="current-password" autofocus value={key} onInput={(e) => setKey(e.target.value)} />
          </div>
          <p class="err" role="alert">{error}</p>
          <button class="btn">Open dashboard</button>
        </div>
      </form>
    </div>
  );
}

function Nav({ route }) {
  const cur = (n) => (route.name === n ? 'page' : undefined);
  return (
    <nav aria-label="Main">
      <div class="brand"><i>K</i>Kids Games</div>
      <a href="#/" aria-current={cur('overview')}>Overview</a>
      <a href="#/players" aria-current={route.name === 'players' || route.name === 'player' ? 'page' : undefined}>Players</a>
      <div class="group">Leaderboards</div>
      {GAMES.map((g) => (
        <a key={g} href={`#/games/${g}`} aria-current={route.name === 'game' && route.game === g ? 'page' : undefined}>{gameName(g)}</a>
      ))}
      <button class="out" onClick={() => session.clear()}>Sign out</button>
    </nav>
  );
}

function Page({ route }) {
  if (route.name === 'players') return <Players />;
  if (route.name === 'player') return <Player key={route.id} id={route.id} />;
  if (route.name === 'game') return <Game key={route.game} game={route.game} />;
  return <Overview />;
}

function App() {
  const [signedIn, setSignedIn] = useState(!!session.key);
  const [route, setRoute] = useState(parseRoute(location.hash));
  useEffect(() => {
    session.onSignOut = () => setSignedIn(false);
    const on = () => setRoute(parseRoute(location.hash));
    addEventListener('hashchange', on);
    return () => removeEventListener('hashchange', on);
  }, []);
  return (
    <>
      {signedIn ? (
        <div class="app">
          <Nav route={route} />
          <main><Page route={route} /></main>
        </div>
      ) : <Login onSignedIn={() => setSignedIn(true)} />}
      <Overlay />
    </>
  );
}

fetch('/v1/games')
  .then((r) => r.json())
  .then((r) => setGames(r.games))
  .catch(() => {})
  .finally(() => render(<App />, document.getElementById('app')));
