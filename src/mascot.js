// "Voi" - Voiceo's mascot: a soft orange gumdrop with a little mic-bulb antenna.
// Pure canvas vector drawing. States: idle | listening | thinking | done | learned | error
(function () {
  const C = {
    top: "#F29A76",
    bottom: "#D96844",
    shade: "rgba(120, 40, 15, 0.16)",
    eye: "#2A1913",
    cheek: "rgba(255, 190, 165, 0.55)",
    bulb: "#FFC9A8",
  };

  function bodyPath(ctx) {
    const l = -30, r = 30, t = -26, b = 26, rt = 30, rb = 15;
    ctx.beginPath();
    ctx.moveTo(l + rt, t);
    ctx.lineTo(r - rt, t);
    ctx.arc(r - rt, t + rt, rt, -Math.PI / 2, 0);
    ctx.lineTo(r, b - rb);
    ctx.arc(r - rb, b - rb, rb, 0, Math.PI / 2);
    ctx.lineTo(l + rb, b);
    ctx.arc(l + rb, b - rb, rb, Math.PI / 2, Math.PI);
    ctx.lineTo(l, t + rt);
    ctx.arc(l + rt, t + rt, rt, Math.PI, Math.PI * 1.5);
    ctx.closePath();
  }

  function eye(ctx, x, y, o) {
    ctx.save();
    ctx.translate(x, y);
    if (o.kind === "happy") {
      ctx.strokeStyle = C.eye; ctx.lineWidth = 3; ctx.lineCap = "round";
      ctx.beginPath(); ctx.arc(0, 2, 5.2, Math.PI * 1.1, Math.PI * 1.9); ctx.stroke();
    } else if (o.kind === "x") {
      ctx.strokeStyle = C.eye; ctx.lineWidth = 2.8; ctx.lineCap = "round";
      ctx.beginPath(); ctx.moveTo(-4.5, -4.5); ctx.lineTo(4.5, 4.5); ctx.moveTo(4.5, -4.5); ctx.lineTo(-4.5, 4.5); ctx.stroke();
    } else {
      const rx = 4.6, ry = Math.max(0.6, o.ry);
      ctx.fillStyle = C.eye;
      ctx.beginPath(); ctx.ellipse(o.lx * 2.6, o.ly * 2.2, rx, ry, 0, 0, Math.PI * 2); ctx.fill();
      if (ry > 2.5) {
        ctx.fillStyle = "rgba(255,255,255,.92)";
        ctx.beginPath(); ctx.arc(o.lx * 2.6 + 1.5, o.ly * 2.2 - ry * 0.35, 1.6, 0, Math.PI * 2); ctx.fill();
      }
    }
    ctx.restore();
  }

  function sparkle(ctx, x, y, r, a) {
    ctx.save();
    ctx.translate(x, y); ctx.globalAlpha = a; ctx.fillStyle = "#FFE6B0";
    ctx.beginPath();
    for (let i = 0; i < 8; i++) {
      const rad = i % 2 ? r * 0.28 : r, ang = (i * Math.PI) / 4;
      ctx.lineTo(Math.cos(ang) * rad, Math.sin(ang) * rad);
    }
    ctx.closePath(); ctx.fill(); ctx.restore();
  }

  // o: {x, y, s, state, t, level, look:[x,y]}  (x,y = center of body, s = scale)
  function draw(ctx, o) {
    const { x, y, s = 1, state = "idle", t = 0, level = 0 } = o;
    const look = o.look || [0, 0];
    ctx.save();
    ctx.translate(x, y);
    ctx.scale(s, s);

    // breathing / voice squash, anchored at the bottom
    const talk = state === "listening" ? Math.min(1, level) : 0;
    const br = 1 + Math.sin(t / 650) * 0.018 + talk * 0.07;
    ctx.translate(0, 26); ctx.scale(1 / Math.sqrt(br), br); ctx.translate(0, -26);

    // antenna
    const wob = state === "listening" ? talk * 7 : state === "thinking" ? Math.sin(t / 160) * 3 : Math.sin(t / 900) * 1.5;
    const droop = state === "error" ? 9 : 0;
    ctx.strokeStyle = "#E2724F"; ctx.lineWidth = 3; ctx.lineCap = "round";
    ctx.beginPath(); ctx.moveTo(-9, -24);
    ctx.quadraticCurveTo(-13, -34 + droop * 0.4, -17 + wob * 0.6 - droop * 0.3, -41 + droop - Math.abs(wob) * 0.3);
    ctx.stroke();
    ctx.fillStyle = C.bulb;
    ctx.beginPath(); ctx.arc(-17 + wob * 0.6 - droop * 0.3, -41 + droop - Math.abs(wob) * 0.3, 4.2 + talk * 1.6, 0, Math.PI * 2); ctx.fill();

    // body
    const g = ctx.createLinearGradient(0, -26, 0, 26);
    g.addColorStop(0, C.top); g.addColorStop(1, C.bottom);
    ctx.fillStyle = g; bodyPath(ctx); ctx.fill();
    ctx.save(); bodyPath(ctx); ctx.clip();
    ctx.fillStyle = C.shade; ctx.beginPath(); ctx.ellipse(8, 30, 44, 17, 0, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = "rgba(255,255,255,.14)"; ctx.beginPath(); ctx.ellipse(-12, -17, 14, 7, -0.4, 0, Math.PI * 2); ctx.fill();
    ctx.restore();

    // cheeks
    const happy = state === "done" || state === "learned";
    ctx.fillStyle = C.cheek; ctx.globalAlpha = happy ? 1 : 0.7;
    ctx.beginPath(); ctx.ellipse(-20, 8, 5.2, 3.4, 0, 0, Math.PI * 2); ctx.ellipse(20, 8, 5.2, 3.4, 0, 0, Math.PI * 2); ctx.fill();
    ctx.globalAlpha = 1;

    // eyes
    let ey = { kind: "dot", ry: 6.3, lx: look[0], ly: look[1] };
    if (state === "idle") {
      const blink = t % 3800 > 3660 ? 0.15 : 1;
      ey.ry = 6.3 * blink;
    } else if (state === "listening") ey.ry = 7.4;
    else if (state === "thinking") { ey.ry = 5; ey.lx = Math.sin(t / 520) * 0.9; ey.ly = -0.9; }
    else if (happy) ey.kind = "happy";
    else if (state === "error") ey.kind = "x";
    eye(ctx, -11, -4, ey); eye(ctx, 11, -4, ey);

    // mouth
    ctx.strokeStyle = C.eye; ctx.fillStyle = C.eye; ctx.lineWidth = 2.6; ctx.lineCap = "round";
    ctx.beginPath();
    if (state === "listening") {
      const open = 1.5 + talk * 7.5;
      ctx.ellipse(0, 11, 3.6 + talk * 1.6, open, 0, 0, Math.PI * 2); ctx.fill();
    } else if (happy) {
      ctx.moveTo(-7, 8); ctx.quadraticCurveTo(0, 18, 7, 8); ctx.stroke();
    } else if (state === "thinking") {
      ctx.moveTo(-4, 12); ctx.lineTo(4, 12); ctx.stroke();
    } else if (state === "error") {
      ctx.moveTo(-6, 15); ctx.quadraticCurveTo(0, 8, 6, 15); ctx.stroke();
    } else {
      ctx.moveTo(-5, 10); ctx.quadraticCurveTo(0, 14.5, 5, 10); ctx.stroke();
    }
    if (state === "learned") {
      sparkle(ctx, 27, -30 - Math.sin(t / 200) * 2, 8, 0.7 + 0.3 * Math.sin(t / 150));
      sparkle(ctx, 36, -18, 4.5, 0.6 + 0.4 * Math.sin(t / 190 + 1));
    }
    ctx.restore();
  }

  function hands(ctx, o) {
    const { x, y, s = 1 } = o;
    ctx.save(); ctx.translate(x, y); ctx.scale(s, s);
    ctx.fillStyle = "#E37A55";
    for (const dx of [-15, 15]) {
      ctx.beginPath(); ctx.ellipse(dx, 0, 6.2, 5, 0, 0, Math.PI * 2); ctx.fill();
    }
    ctx.restore();
  }

  window.Voi = { draw, hands };
})();
