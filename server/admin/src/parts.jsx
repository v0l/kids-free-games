import { useState, useEffect } from 'preact/hooks';

export function Avatar({ player, big }) {
  const hue = (player.avatar * 47 + 250) % 360;
  return (
    <span class={`av${big ? ' big' : ''}`} style={`background:hsl(${hue} 62% 52%)`} aria-hidden="true">
      {(player.name || '?')[0].toUpperCase()}
    </span>
  );
}

export function AvatarPicker({ value, onChange }) {
  return (
    <div class="avatars">
      {Array.from({ length: 12 }, (_, i) => (
        <button type="button" key={i} aria-pressed={i === value} aria-label={`Avatar ${i + 1}`} onClick={() => onChange(i)}>
          <Avatar player={{ avatar: i, name: String.fromCharCode(65 + i) }} />
        </button>
      ))}
    </div>
  );
}

export function PlayerForm({ player, state }) {
  const [name, setName] = useState(player ? player.name : '');
  const [avatar, setAvatar] = useState(player ? player.avatar : 0);
  state.read = () => ({ name, avatar });
  return (
    <>
      <div class="field">
        <label for="pn">Name</label>
        <input id="pn" type="text" maxLength={12} autocomplete="off" value={name} onInput={(e) => setName(e.target.value)} />
        <span class="mute">Letters only, up to 12.</span>
      </div>
      <div class="field">
        <label>Avatar</label>
        <AvatarPicker value={avatar} onChange={setAvatar} />
      </div>
    </>
  );
}

export function usePromise(load, deps) {
  const [state, setState] = useState({ loading: true });
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let live = true;
    setState((s) => ({ ...s, loading: true }));
    load().then(
      (data) => live && setState({ data }),
      (error) => live && setState({ error }),
    );
    return () => { live = false; };
  }, [...deps, tick]);
  return [state, () => setTick((t) => t + 1)];
}

export function Loaded({ state, children }) {
  if (state.error) {
    return (
      <div class="empty">
        {state.error.status === 404 ? 'That one is gone.' : state.error.message} <a href="#/">Back to overview</a>
      </div>
    );
  }
  if (!state.data) return <div class="loading">Loading</div>;
  return children(state.data);
}

export function LinkCode({ link, name }) {
  return (
    <>
      <p style="margin:0 0 6px">
        Enter this on a device to sign in as <b>{name}</b>. It works once and expires in {Math.round(link.expires_in / 60)} minutes.
      </p>
      <div class="code">{link.code}</div>
    </>
  );
}
