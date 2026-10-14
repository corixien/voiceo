// Pixel mascot "Voi". Draws onto a canvas; states: idle | listening | thinking | done | error
(function () {
  const BODY = [
    "...ooooo...",
    "..ooooooo..",
    ".ooooooooo.",
    "ooooooooooo",
    ".ooooooooo.",
    "..ooooooo..",
    "..oo...oo..",
  ];
  const ORANGE = "#D97757";
  const EYE = "#2B1B14";

  const eyes = {
    idle: (blink) => (blink ? [[3, 3], [7, 3]] : [[3, 2], [3, 3], [7, 2], [7, 3]]),
    listening: () => [[3, 2], [3, 3], [7, 2], [7, 3]],
    thinking: (_, t) => {
      const dx = Math.round(Math.sin(t / 260));
      return [[3 + dx, 2], [3 + dx, 3], [7 + dx, 2], [7 + dx, 3]];
    },
    done: () => [[2, 3], [3, 2], [4, 3], [6, 3], [7, 2], [8, 3]],
    error: () => [[2, 2], [4, 2], [3, 3], [2, 4], [4, 4], [6, 2], [8, 2], [7, 3], [6, 4], [8, 4]],
  };

  window.drawMascot = function (canvas, state, t, level) {
    const ctx = canvas.getContext("2d");
    const px = Math.floor(canvas.width / 11);
    const ox = Math.floor((canvas.width - 11 * px) / 2);
    const rows = BODY.length;
    const hop = state === "listening" ? Math.round(Math.min(1, level * 9) * 2) : 0;
    const oy = canvas.height - rows * px - px - hop * px * 0.5;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.imageSmoothingEnabled = false;

    ctx.fillStyle = ORANGE;
    BODY.forEach((row, y) => {
      for (let x = 0; x < row.length; x++) {
        if (row[x] !== "o") continue;
        let yy = y;
        // legs wiggle while thinking
        if (state === "thinking" && y === 6) yy += (x < 5) === (Math.floor(t / 180) % 2 === 0) ? 0 : 0;
        ctx.fillRect(ox + x * px, Math.round(oy + yy * px), px, px);
      }
    });

    const blink = state === "idle" && t % 3600 > 3440;
    const e = (eyes[state] || eyes.idle)(blink, t);
    ctx.fillStyle = EYE;
    for (const [x, y] of e) ctx.fillRect(ox + x * px, Math.round(oy + y * px), px, px);

    if (state === "listening") {
      // mouth opens with voice level
      const open = Math.min(2, Math.round(level * 14));
      if (open > 0) {
        ctx.fillRect(ox + 4 * px, Math.round(oy + 4 * px), 3 * px, Math.max(1, open) * px * 0.6);
      }
    }
    if (state === "thinking") {
      // three dots above the head
      const n = 1 + (Math.floor(t / 280) % 3);
      ctx.fillStyle = ORANGE;
      for (let i = 0; i < n; i++) {
        ctx.fillRect(ox + (3 + i * 2) * px, Math.round(oy - px * 1.6), Math.max(2, px - 1), Math.max(2, px - 1));
      }
    }
  };
})();
