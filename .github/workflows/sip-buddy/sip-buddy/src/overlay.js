// Sip Buddy overlay: walks the character in, shows the reminder, walks her off.
(() => {
  const T = window.__TAURI__;
  const invoke = (cmd, args) => (T ? T.core.invoke(cmd, args) : Promise.resolve(null));

  const walker = document.getElementById('walker');
  const girl = document.getElementById('girl');
  const bubble = document.getElementById('bubble');
  const title = document.getElementById('bubble-title');
  const sub = document.getElementById('bubble-sub');
  const actions = document.getElementById('bubble-actions');
  const sparkles = document.getElementById('sparkles');
  const btnDone = document.getElementById('btn-done');
  const btnSnooze = document.getElementById('btn-snooze');

  const SPEED = 125;          // px per second
  const IGNORE_AFTER = 120e3; // auto-snooze if nobody responds for 2 minutes
  let x = -200;
  let busy = false;
  let ignoreTimer = null;
  let snoozeMinutes = 10;

  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const setX = (v) => { x = v; walker.style.transform = `translateX(${v}px)`; };
  const setPose = (pose) => { girl.setAttribute('class', pose); };

  function walkTo(target) {
    setPose('walking');
    return new Promise((resolve) => {
      const start = x;
      const dist = Math.abs(target - start);
      const dur = (dist / SPEED) * 1000;
      const t0 = performance.now();
      const step = (now) => {
        const p = Math.min(1, (now - t0) / dur);
        setX(start + (target - start) * p);
        if (p < 1) requestAnimationFrame(step); else resolve();
      };
      requestAnimationFrame(step);
    });
  }

  // Tell the app which area is clickable; everywhere else clicks pass through.
  function reportHitArea(on) {
    if (!on) return invoke('set_hit_rect', { rect: null });
    const a = bubble.getBoundingClientRect();
    const b = girl.getBoundingClientRect();
    const pad = 8;
    const left = Math.min(a.left, b.left) - pad;
    const top = Math.min(a.top, b.top) - pad;
    const right = Math.max(a.right, b.right) + pad;
    const bottom = Math.max(a.bottom, b.bottom) + pad;
    return invoke('set_hit_rect', { rect: [left, top, right - left, bottom - top] });
  }

  function chime() {
    try {
      const ctx = new (window.AudioContext || window.webkitAudioContext)();
      [880, 1318.5].forEach((f, i) => {
        const o = ctx.createOscillator();
        const g = ctx.createGain();
        o.type = 'sine';
        o.frequency.value = f;
        const t = ctx.currentTime + i * 0.16;
        g.gain.setValueAtTime(0.0001, t);
        g.gain.exponentialRampToValueAtTime(0.12, t + 0.02);
        g.gain.exponentialRampToValueAtTime(0.0001, t + 0.5);
        o.connect(g).connect(ctx.destination);
        o.start(t);
        o.stop(t + 0.55);
      });
      setTimeout(() => ctx.close(), 1200);
    } catch (_) { /* sound is optional */ }
  }

  function burst() {
    const icons = ['💧', '💦', '💧', '✨', '💗', '💧'];
    icons.forEach((icon, i) => {
      const d = document.createElement('span');
      d.className = 'drop';
      d.textContent = icon;
      const angle = (-150 + i * 24) * (Math.PI / 180);
      d.style.setProperty('--dx', `${Math.cos(angle) * 70}px`);
      d.style.setProperty('--dy', `${Math.sin(angle) * 70}px`);
      sparkles.appendChild(d);
      setTimeout(() => d.remove(), 1200);
    });
  }

  async function startReminder(info) {
    if (busy) return;
    busy = true;
    snoozeMinutes = info.snooze_minutes || 10;
    const next = (info.glasses || 0) + 1;
    title.textContent = pick(['Time for a sip!', 'Water break!', 'Stay hydrated!', 'Sip sip, hooray!']);
    sub.textContent = `Glass ${next} of ${info.goal || 8} today`;
    btnSnooze.textContent = `Snooze ${snoozeMinutes} min`;
    actions.classList.remove('hidden');
    bubble.classList.remove('show');

    setX(-200);
    const stopAt = Math.max(80, Math.min(window.innerWidth - 300, window.innerWidth * 0.68));
    await walkTo(stopAt);
    setPose('standing');
    if (info.sound !== false) chime();
    await sleep(250);
    bubble.classList.add('show');
    await sleep(400);
    reportHitArea(true);
    ignoreTimer = setTimeout(() => finish('ignored'), IGNORE_AFTER);
  }

  async function finish(kind) {
    clearTimeout(ignoreTimer);
    actions.classList.add('hidden');
    reportHitArea(false);

    if (kind === 'done') {
      const res = await invoke('reminder_done');
      const glasses = res ? res.glasses : 1;
      const goal = res ? res.goal : 8;
      title.textContent = glasses >= goal ? 'Daily goal reached! 🎉' : 'Yay, well done!';
      sub.textContent = `${glasses} of ${goal} glasses today 💧`;
      setPose('cheer');
      burst();
      await sleep(1700);
    } else {
      await invoke('reminder_snooze');
      title.textContent = kind === 'ignored' ? "I'll check back soon!" : 'Okay! See you soon';
      sub.textContent = `Back in ${snoozeMinutes} minutes`;
      setPose('wave');
      await sleep(1600);
    }

    bubble.classList.remove('show');
    await sleep(200);
    await walkTo(window.innerWidth + 60);
    setPose('walking');
    busy = false;
    await invoke('overlay_finished');
  }

  function pick(list) { return list[Math.floor(Math.random() * list.length)]; }

  btnDone.addEventListener('click', () => finish('done'));
  btnSnooze.addEventListener('click', () => finish('snooze'));

  if (T) {
    T.event.listen('start-reminder', (e) => startReminder(e.payload || {}));
    invoke('overlay_ready');
  } else {
    // Browser preview mode (no desktop app around)
    window.addEventListener('load', () => startReminder({ glasses: 3, goal: 8, sound: false }));
  }
})();
