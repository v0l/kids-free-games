import { render } from 'preact';
import { useState, useEffect, useRef } from 'preact/hooks';

let pushToast = () => {};
let pushDialog = () => {};

export function toast(message, bad = false) {
  pushToast({ message, bad, id: Math.random() });
}

export async function act(fn, done) {
  try {
    const result = await fn();
    if (done) toast(done);
    return result;
  } catch (e) {
    toast(e.message, true);
    throw e;
  }
}

export function openDialog(spec) {
  return new Promise((resolve) => pushDialog({ spec, resolve }));
}

export function confirmDialog(title, text, confirm) {
  return openDialog({ title, danger: true, confirm, body: () => <p style="margin:0">{text}</p> });
}

function DialogView({ spec, resolve, onDone }) {
  const ref = useRef();
  const [busy, setBusy] = useState(false);
  const state = useRef({});
  useEffect(() => {
    ref.current.showModal();
    const first = ref.current.querySelector('input,textarea,select');
    if (first) first.focus();
  }, []);
  const finish = (value) => {
    ref.current.close();
    resolve(value);
    onDone();
  };
  const submit = async (e) => {
    e.preventDefault();
    setBusy(true);
    try {
      const value = spec.onOk ? await spec.onOk(state.current) : true;
      finish(value === undefined ? true : value);
    } catch {
      setBusy(false);
    }
  };
  const Body = spec.body;
  return (
    <dialog ref={ref} class={spec.wide ? 'wide' : ''} onCancel={() => { resolve(null); onDone(); }}>
      <form onSubmit={submit}>
        <div class={`top${spec.danger ? '' : ' ok'}`}>{spec.title}</div>
        <div class="body"><Body state={state.current} /></div>
        <div class="foot">
          <button type="button" class="btn quiet" onClick={() => finish(null)}>{spec.cancel || 'Cancel'}</button>
          {spec.confirm !== '' && (
            <button class={`btn${spec.danger ? ' danger' : ''}`} disabled={busy}>{spec.confirm || 'Save'}</button>
          )}
        </div>
      </form>
    </dialog>
  );
}

export function Overlay() {
  const [toasts, setToasts] = useState([]);
  const [dialogs, setDialogs] = useState([]);
  useEffect(() => {
    pushToast = (t) => {
      setToasts((x) => [...x, t]);
      setTimeout(() => setToasts((x) => x.filter((y) => y !== t)), t.bad ? 5000 : 2600);
    };
    pushDialog = (d) => setDialogs((x) => [...x, { ...d, id: Math.random() }]);
  }, []);
  return (
    <>
      {dialogs.map((d) => (
        <DialogView key={d.id} spec={d.spec} resolve={d.resolve} onDone={() => setDialogs((x) => x.filter((y) => y !== d))} />
      ))}
      <div id="toast" aria-live="polite">
        {toasts.map((t) => <div key={t.id} class={`toast${t.bad ? ' bad' : ''}`}>{t.message}</div>)}
      </div>
    </>
  );
}

export { render };
