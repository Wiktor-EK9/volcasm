// Hero animation: a sample is recorded into slot 1 (the app's slot and
// record panels), copied to slots 2-4 as cards standing one behind another
// in isometric view, then each card is trimmed to a different section and
// sent off the screen, turning into seven-segment data like the transfer
// animation in the app (volcasm-app/src/ui/transfer_dialog.rs).
// Drawing follows the app: waveform.rs (bars 3 px on a 4 px pitch, levels in
// 3 px steps, red start/end handles with 8 px flags), record_panel.rs (label,
// level meter, red STOP), style.rs (fonts, colors).
(() => {
  "use strict";
  const canvas = document.getElementById("hero-anim");
  if (!canvas || !canvas.getContext) return;
  const ctx = canvas.getContext("2d");
  const VW = 560, VH = 520;

  // ---------- theme colors, read from the CSS tokens ----------
  let C = {};
  const readColors = () => {
    const s = getComputedStyle(document.documentElement);
    for (const k of ["accent", "panel", "background", "tile-empty", "tile-filled", "text", "text-dim", "wave"]) {
      C[k] = s.getPropertyValue("--" + k).trim();
    }
  };
  readColors();
  new MutationObserver(readColors).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  matchMedia("(prefers-color-scheme: light)").addEventListener("change", readColors);
  const BORDER = "rgb(70,70,70)"; // waveform frame, Color32::from_gray(70)

  // ---------- fonts ----------
  const BRAND = 'Overpass, "Barlow Semi Condensed", sans-serif';
  const UI = '"Barlow Semi Condensed", Overpass, sans-serif';
  const SEG = '"DSEG7 Classic Mini", monospace';
  const ICON = "Phosphor";

  // ---------- easing and helpers ----------
  const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
  const lerp = (a, b, t) => a + (b - a) * t;
  const span = (t, a, b) => clamp((t - a) / (b - a));
  const easeInOut = t => (t < .5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);
  const easeOut = t => 1 - Math.pow(1 - t, 3);
  const easeIn = t => t * t * t;
  let seed = 7;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646;

  // ---------- the sample: a little drum-like loop, peaks per bar ----------
  const CARD_W = 320, PAD = 14;
  const BOX_X = PAD, BOX_Y = 70, BOX_W = CARD_W - 2 * PAD, BOX_H = 96;
  const BARS = Math.floor(BOX_W / 4);
  // Shaped like one bar of the amen break: kicks and snares as sharp
  // spikes over a dense ride/hi-hat body, ghost notes in between.
  const peaks = (() => {
    const out = [];
    // [step of 16, peak, decay rate]: K . K . S . . s . s K K . S . .
    const hits = [[0, .8, 7], [2, .72, 8], [4, 1, 4.5], [7, .5, 12], [9, .55, 12], [10, .74, 8], [11, .68, 8], [13, .98, 4.5]];
    let r = 11;
    const n = () => ((r = (r * 48271) % 2147483647) / 2147483647);
    for (let i = 0; i < BARS; i++) {
      const x = i / BARS * 16;
      // the ride: a busy body that swells a little after every eighth
      let a = 0.2 + 0.07 * Math.exp(-(x % 2) * 2.5) + 0.1 * (n() - 0.5);
      for (const [step, peak, decay] of hits) {
        const d = x - step;
        if (d >= 0) a = Math.max(a, peak * Math.exp(-d * decay / 4) * (0.85 + 0.15 * n()));
      }
      a = Math.min(1, a);
      out.push([-(a * (0.88 + 0.12 * n())), a * (0.9 + 0.1 * n())]);
    }
    return out;
  })();

  // ---------- timeline (seconds) ----------
  const T = {
    rec: [0.4, 3.6],      // recording, bars appear
    press: [3.4, 3.7],    // STOP pressed
    collapse: [3.7, 4.4], // record panel folds away
    iso: [4.4, 5.4],      // flat -> isometric
    copy0: 5.5, copyDur: 0.9, copyGap: 0.35,
    trim0: 7.2, trimGap: 2.1,
    loop: 17.4,
  };
  const SECTIONS = [[0, .25], [.25, .5], [.5, .75], [.75, 1]];
  const DATA_CHARS = "0123456789ABCDEFHJLNOPRSUYbcdhnortu-";
  const SIZES = [13, 19, 28];

  // isometric frame for upright cards: local x -> down-right, local y -> down
  const ISO = [Math.cos(Math.PI / 6), Math.sin(Math.PI / 6), 0, 1];
  const BEHIND = [Math.cos(Math.PI / 6), -Math.sin(Math.PI / 6)]; // away from the viewer
  const STEP = 50;

  // ---------- particles: seven-segment data ----------
  let particles = [];
  const emit = (x, y, dx, dy) => {
    const ang = Math.atan2(dy, dx) + (rnd() - .5) * 0.9;
    const sp = 70 + rnd() * 120;
    let text = "";
    for (let i = 0, n = 1 + Math.floor(rnd() * 3); i < n; i++) text += DATA_CHARS[Math.floor(rnd() * DATA_CHARS.length)];
    particles.push({ x, y, vx: Math.cos(ang) * sp, vy: Math.sin(ang) * sp - 20, a: ang, text, size: SIZES[Math.floor(rnd() * 3)], age: 0, life: 0.8 + rnd() * 0.5 });
  };
  const stepParticles = dt => {
    for (const p of particles) {
      p.age += dt;
      p.x += p.vx * dt; p.y += p.vy * dt;
      p.vx *= 1 - 0.8 * dt; p.vy *= 1 - 0.8 * dt;
    }
    particles = particles.filter(p => p.age < p.life);
  };
  const drawParticles = () => {
    ctx.textBaseline = "middle"; ctx.textAlign = "center";
    for (const p of particles) {
      const t = p.age / p.life;
      ctx.globalAlpha = Math.min(1, t / 0.15) * Math.min(1, (1 - t) / 0.35);
      ctx.fillStyle = C.accent;
      ctx.font = `${p.size}px ${SEG}`;
      ctx.save(); ctx.translate(p.x, p.y); ctx.rotate(p.a); ctx.fillText(p.text, 0, 0); ctx.restore();
    }
    ctx.globalAlpha = 1;
  };

  // ---------- drawing the app's widgets in card-local coordinates ----------
  const caps = s => s.toUpperCase();
  const waveform = (reveal, range, handles) => {
    ctx.fillStyle = C.background; ctx.fillRect(BOX_X, BOX_Y, BOX_W, BOX_H);
    const mid = BOX_Y + BOX_H / 2, half = BOX_H / 2 - 2;
    const snap = v => Math.min(half, Math.max(1, Math.ceil(v * half / 3)) * 3);
    const sx = BOX_X + range[0] * BOX_W, ex = BOX_X + range[1] * BOX_W;
    const shown = Math.floor(BARS * reveal);
    for (let i = 0; i < shown; i++) {
      const [lo, hi] = peaks[i];
      const left = BOX_X + i * 4, center = left + 1.5;
      const inside = center >= sx && center <= ex;
      ctx.fillStyle = inside ? C.wave : C["text-dim"];
      ctx.globalAlpha = inside ? 1 : 0.5;
      const top = mid - snap(Math.max(0, hi)), bot = mid + snap(Math.max(0, -lo));
      ctx.fillRect(left, top, 3, bot - top);
    }
    ctx.globalAlpha = 0.4; ctx.fillStyle = C["text-dim"]; ctx.fillRect(BOX_X, mid - 0.5, BOX_W, 1);
    ctx.globalAlpha = 1;
    if (handles > 0) {
      ctx.globalAlpha = handles; ctx.fillStyle = C.accent;
      ctx.fillRect(sx - 1, BOX_Y, 2, BOX_H); ctx.fillRect(sx, BOX_Y, 8, 8);
      ctx.fillRect(ex - 1, BOX_Y, 2, BOX_H); ctx.fillRect(ex - 8, BOX_Y, 8, 8);
      ctx.globalAlpha = 1;
    }
    ctx.strokeStyle = BORDER; ctx.lineWidth = 1; ctx.strokeRect(BOX_X + .5, BOX_Y + .5, BOX_W - 1, BOX_H - 1);
  };
  const caption = (icon, label, x, y, right) => {
    ctx.textBaseline = "middle"; ctx.textAlign = "left"; ctx.fillStyle = C["text-dim"];
    ctx.font = `13px ${ICON}`; ctx.fillText(icon, x, y);
    ctx.font = `500 13px ${UI}`; ctx.fillText(caps(label), x + 18, y + 1);
    const end = x + 18 + ctx.measureText(caps(label)).width + 8;
    ctx.globalAlpha = 0.6; ctx.fillStyle = C["tile-filled"]; ctx.fillRect(end, y, right - end, 1); ctx.globalAlpha = 1;
  };

  // One slot card. o: {slot, reveal, range, handles, rec (0..1 record extras), recTime, level, press, clipL, clipR, alpha}
  const CARD_H = 180, REC_H = 318;
  const card = o => {
    const h = lerp(CARD_H, REC_H, o.rec);
    ctx.save();
    ctx.beginPath(); ctx.rect(o.clipL, -6, o.clipR - o.clipL + 6, h + 6); ctx.clip();
    ctx.globalAlpha = o.alpha;
    // thickness: an edge slab behind the face
    ctx.fillStyle = C["tile-empty"];
    roundRect(o.clipL + 4, -4, o.clipR - o.clipL, h, 3); ctx.fill();
    ctx.fillStyle = C.panel; roundRect(o.clipL, 0, o.clipR - o.clipL, h, 3); ctx.fill();
    ctx.restore();

    ctx.save();
    ctx.beginPath(); ctx.rect(o.clipL, 0, o.clipR - o.clipL, h); ctx.clip();
    ctx.globalAlpha = o.alpha;
    ctx.fillStyle = C.text; ctx.textBaseline = "top"; ctx.textAlign = "left";
    ctx.font = `700 26px ${BRAND}`;
    const hx = Math.max(PAD, o.clipL + PAD);
    // "slot 1" while recording, then the recording's name
    if (o.named < 1) { ctx.globalAlpha = o.alpha * (1 - o.named); ctx.fillText("slot " + o.slot, hx, 14); }
    if (o.named > 0) { ctx.globalAlpha = o.alpha * o.named; ctx.fillText("breakbeat " + o.slot, hx, 14); }
    ctx.globalAlpha = o.alpha;
    caption("", "waveform", PAD, 56, CARD_W - PAD);
    waveform(o.reveal, o.range, o.handles);
    if (o.rec > 0) {
      ctx.globalAlpha = o.alpha * o.rec;
      ctx.fillStyle = C["tile-filled"]; ctx.globalAlpha *= .6; ctx.fillRect(PAD, CARD_H, BOX_W, 1); ctx.globalAlpha = o.alpha * o.rec;
      ctx.fillStyle = C.text; ctx.textBaseline = "top";
      ctx.font = `700 22px ${BRAND}`; ctx.fillText("record", PAD, CARD_H + 12);
      ctx.font = `500 15px ${UI}`;
      ctx.fillText(caps(`recording into slot ${o.slot} · ${o.recTime.toFixed(1)} / 66 s`), PAD, CARD_H + 46);
      // level meter, record_panel.rs
      ctx.fillStyle = C.background; roundRect(PAD, CARD_H + 72, BOX_W, 14, 2); ctx.fill();
      ctx.fillStyle = o.level >= 0.99 ? C.accent : C.wave; roundRect(PAD, CARD_H + 72, BOX_W * o.level, 14, 2); ctx.fill();
      // STOP button: accent fill, interface text, 18 px
      ctx.fillStyle = C.accent; roundRect(PAD, CARD_H + 96, BOX_W, 30, 3); ctx.fill();
      if (o.press > 0) { ctx.fillStyle = "#fff"; ctx.globalAlpha = o.alpha * o.rec * 0.25 * o.press; roundRect(PAD, CARD_H + 96, BOX_W, 30, 3); ctx.fill(); ctx.globalAlpha = o.alpha * o.rec; }
      ctx.fillStyle = C.text; ctx.textAlign = "center"; ctx.textBaseline = "middle";
      ctx.font = `18px ${ICON}`; ctx.fillText("", CARD_W / 2 - 28, CARD_H + 111);
      ctx.font = `500 18px ${UI}`; ctx.fillText("STOP", CARD_W / 2 + 8, CARD_H + 112);
    }
    ctx.restore();
    return h;
  };
  const roundRect = (x, y, w, h, r) => {
    ctx.beginPath();
    if (w <= 0 || h <= 0) return;
    r = Math.min(r, w / 2, h / 2);
    ctx.moveTo(x + r, y); ctx.arcTo(x + w, y, x + w, y + h, r); ctx.arcTo(x + w, y + h, x, y + h, r);
    ctx.arcTo(x, y + h, x, y, r); ctx.arcTo(x, y, x + w, y, r); ctx.closePath();
  };

  // ---------- frame ----------
  // Flat placement (record), then isometric placement of card 0.
  const FLAT_E = (VW - CARD_W) / 2;
  const ISO_E = 280 - 75 - ISO[0] * CARD_W / 2, ISO_F = 300 + 43 - (ISO[1] * CARD_W / 2 + CARD_H / 2);
  let lastEdges = [];

  const frame = (t, dt) => {
    ctx.setTransform(scale, 0, 0, scale, 0, 0);
    ctx.clearRect(0, 0, VW, VH);

    const fadeIn = span(t, 0, 0.4);
    const rec = 1 - easeInOut(span(t, ...T.collapse));
    const k = easeInOut(span(t, ...T.iso));
    const reveal = span(t, ...T.rec);
    const recTime = 2.8 * reveal;
    const bar = Math.min(BARS - 1, Math.floor(BARS * reveal));
    const level = reveal < 1 ? clamp(peaks[bar][1] * 1.05) : 0;
    const press = Math.sin(Math.PI * span(t, ...T.press));
    const handles = easeOut(span(t, T.collapse[0], T.collapse[1]));

    const h0 = lerp(CARD_H, REC_H, rec);
    const flatF = (VH - h0) / 2;
    const m = [lerp(1, ISO[0], k), lerp(0, ISO[1], k), 0, 1];
    const e0 = lerp(FLAT_E, ISO_E, k), f0 = lerp(flatF, ISO_F, k);

    // cards from the back to the front
    const edges = [];
    for (let i = 3; i >= 0; i--) {
      // copy: slide from behind card 0 to place i
      let place = i;
      if (i > 0) {
        const c = easeOut(span(t, T.copy0 + (i - 1) * T.copyGap, T.copy0 + (i - 1) * T.copyGap + T.copyDur));
        if (c <= 0) continue;
        place = i * c;
      }
      const t0 = T.trim0 + i * T.trimGap;
      const [s, e] = SECTIONS[i];
      const tr = easeInOut(span(t, t0, t0 + 0.6));
      const range = [lerp(0, s, tr), lerp(1, e, tr)];
      const crop = easeInOut(span(t, t0 + 0.6, t0 + 1.1));
      const sx = BOX_X + s * BOX_W - 6, ex = BOX_X + e * BOX_W + 6;
      let clipL = lerp(0, sx, crop), clipR = lerp(CARD_W, ex, crop);
      const th = easeIn(span(t, t0 + 1.1, t0 + 2.0));
      if (th >= 1) continue;
      const slide = th * 360;
      const wipe = lerp(clipL, clipR, th);
      const prevL = clipL;
      clipL = wipe;

      const ox = BEHIND[0] * STEP * place, oy = BEHIND[1] * STEP * place;
      const tx = m[0] * slide, ty = m[1] * slide;
      const E = e0 + ox + tx, F = f0 + oy + ty;
      ctx.setTransform(scale * m[0], scale * m[1], scale * m[2], scale * m[3], scale * E, scale * F);
      card({ slot: i + 1, named: i === 0 ? handles : 1, reveal: i === 0 ? reveal : 1, range, handles: i === 0 ? handles : 1,
        rec: i === 0 ? rec : 0, recTime, level, press, clipL, clipR, alpha: i === 0 ? fadeIn : 1 });

      // edges that dissolve into data: [local x, direction sign]
      const toScreen = (x, y) => [m[0] * x + m[2] * y + E, m[1] * x + m[3] * y + F];
      edges.push({ id: i, L: crop > 0 && crop < 1 || th > 0 ? clipL : null, R: crop > 0 && crop < 1 ? clipR : null, toScreen, prevL });
    }
    ctx.setTransform(scale, 0, 0, scale, 0, 0);

    // emit particles along the edges that moved since the last frame
    for (const ed of edges) {
      const before = lastEdges.find(x => x.id === ed.id);
      if (!before) continue;
      const out = [m[0], m[1]];
      for (const side of ["L", "R"]) {
        if (ed[side] == null || before[side] == null) continue;
        const moved = Math.abs(ed[side] - before[side]);
        const n = Math.min(2, Math.floor(moved / 8 + rnd()));
        const sign = side === "L" ? -1 : 1;
        for (let j = 0; j < n; j++) {
          const [x, y] = ed.toScreen(ed[side], rnd() * CARD_H);
          emit(x, y, out[0] * sign, out[1] * sign);
        }
      }
    }
    lastEdges = edges;

    stepParticles(dt);
    drawParticles();
  };

  // ---------- loop ----------
  let scale = 1;
  const resize = () => {
    const w = canvas.clientWidth || VW;
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(w * dpr * VH / VW);
    scale = canvas.width / VW;
  };
  addEventListener("resize", resize);

  const still = matchMedia("(prefers-reduced-motion: reduce)");
  let visible = true, start = null, last = null, raf = 0, prevT = 0;
  const tick = now => {
    raf = 0;
    if (start === null) start = now;
    const dt = last === null ? 0 : Math.min(0.05, (now - last) / 1000);
    last = now;
    const t = ((now - start) / 1000) % T.loop;
    if (t < prevT) { particles = []; lastEdges = []; }
    prevT = t;
    frame(t, dt);
    if (visible && !still.matches) raf = requestAnimationFrame(tick);
  };
  const drawStill = () => { particles = []; lastEdges = []; frame(7.15, 0); };
  const run = () => {
    if (still.matches) { drawStill(); return; }
    if (!raf && visible) { last = null; raf = requestAnimationFrame(tick); }
  };
  new IntersectionObserver(es => { visible = es[0].isIntersecting; if (visible) run(); }).observe(canvas);
  still.addEventListener("change", run);
  new MutationObserver(() => { if (still.matches) drawStill(); }).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

  const fonts = [`700 26px ${BRAND}`, `500 15px ${UI}`, `13px ${SEG}`, `13px ${ICON}`];
  Promise.all(fonts.map(f => document.fonts.load(f, f.includes(ICON) ? "" : "a0"))).catch(() => {}).then(() => { resize(); run(); });
})();
