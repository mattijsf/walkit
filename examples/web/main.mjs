// SPDX-License-Identifier: GPL-3.0-or-later
// A page that shows a Winamp skin with walkit's embedding API: one canvas per window, a small
// player of its own, and the pointer passed to the engine.
import { loadWalkit } from "./walkit-web/embed.mjs";

const stage = document.getElementById("stage");
const hint = document.getElementById("hint");

// The page's player: a fixed playlist and a clock. The engine only shows what it is told.
const tracks = [
  { title: "walkit - Example track", lengthMs: 215000 },
  { title: "walkit - Another track", lengthMs: 184000 },
];
const player = { status: "stopped", index: 0, startedAt: 0, positionMs: 0, volume: 200 };

function positionMs(now) {
  return player.status === "playing" ? player.positionMs + (now - player.startedAt) : player.positionMs;
}

function command(name, value, now = performance.now()) {
  player.positionMs = positionMs(now);
  player.startedAt = now;
  switch (name) {
    case "play": player.status = "playing"; break;
    case "pause": player.status = player.status === "paused" ? "playing" : "paused"; break;
    case "stop": player.status = "stopped"; player.positionMs = 0; break;
    case "next": player.index = (player.index + 1) % tracks.length; player.positionMs = 0; break;
    case "previous": player.index = (player.index + tracks.length - 1) % tracks.length; player.positionMs = 0; break;
    case "play_track": player.index = value; player.positionMs = 0; player.status = "playing"; break;
    case "seek": player.positionMs = value; break;
    case "volume": player.volume = value; break;
  }
}

let skin = null;
let defaults = null;
const canvases = new Map(); // window id -> canvas

const walkit = await loadWalkit({
  invalidate: () => skin?.invalidate(),
  player: (name, value) => command(name, value),
  action: (action, param) => console.log("action", action, param),
  log: (line) => console.log(line),
});

async function open(file) {
  skin?.free();
  for (const canvas of canvases.values()) canvas.remove();
  canvases.clear();
  const bytes = new Uint8Array(await file.arrayBuffer());
  // a modern skin sits over Winamp's default element set; a classic one needs none
  const modern = /\.wal$/i.test(file.name);
  skin = await walkit.openSkin(bytes, { name: file.name.replace(/\.\w+$/, ""), defaults: modern ? defaults : null });
  skin.rt.setFree(true, stage.clientWidth, stage.clientHeight);
  skin.rt.setPlaylist(tracks);
  // a classic skin opens with its main window alone; show the equalizer and the playlist too
  skin.rt.openWindow("equalizer");
  skin.rt.openWindow("pledit");
  hint.style.display = "none";
}

// One canvas per window, at the window's rect, so the page between windows stays clickable.
function canvasFor(window) {
  let canvas = canvases.get(window.id);
  if (!canvas) {
    canvas = document.createElement("canvas");
    canvases.set(window.id, canvas);
  }
  const dpr = devicePixelRatio;
  const { left, top, width, height } = window.rect;
  Object.assign(canvas.style, { left: `${left}px`, top: `${top}px`, width: `${width}px`, height: `${height}px` });
  if (canvas.width !== Math.round(width * dpr)) canvas.width = Math.round(width * dpr);
  if (canvas.height !== Math.round(height * dpr)) canvas.height = Math.round(height * dpr);
  stage.append(canvas); // bottom to top: append in the order the windows come
  return canvas;
}

function frame(now) {
  if (skin) {
    skin.rt.setPlayerState({
      status: player.status,
      positionMs: Math.min(positionMs(now), tracks[player.index].lengthMs),
      lengthMs: tracks[player.index].lengthMs,
      title: tracks[player.index].title,
      trackIndex: player.index,
      volume: player.volume,
    });
    if (player.status === "playing") skin.rt.setVis(fakeVis(now));
    if (skin.tick(now)) {
      const shown = new Set();
      for (const w of skin.windows()) {
        skin.paint(canvasFor(w), w, devicePixelRatio);
        shown.add(w.id);
      }
      for (const [id, canvas] of canvases) {
        if (!shown.has(id)) {
          canvas.remove();
          canvases.delete(id);
        }
      }
    }
  }
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);

// Something for the analyzer and the scope to show while the clock runs.
function fakeVis(now) {
  const t = now / 1000;
  const spectrum = new Float32Array(75).map((_, i) => 0.5 + 0.5 * Math.sin(t * 3 + i / 6) * Math.cos(t + i / 20));
  const waveform = new Float32Array(576).map((_, i) => 0.6 * Math.sin(i / 9 + t * 8) * Math.sin(t));
  const level = 0.5 + 0.4 * Math.sin(t * 2);
  return { spectrum, waveform, vu: [level, 1 - level] };
}

// The pointer, in stage pixels: the engine's viewport.
function at(event) {
  const r = stage.getBoundingClientRect();
  return [Math.round(event.clientX - r.left), Math.round(event.clientY - r.top)];
}
stage.addEventListener("pointerdown", (e) => {
  if (!skin) return;
  stage.setPointerCapture(e.pointerId);
  skin.rt.pointerDown(...at(e), e.pointerId);
});
stage.addEventListener("pointermove", (e) => skin?.rt.pointerMove(...at(e)));
stage.addEventListener("pointerup", (e) => skin?.rt.pointerUp(...at(e)));
stage.addEventListener("dblclick", (e) => skin?.rt.doubleTap(...at(e)));
stage.addEventListener("contextmenu", (e) => e.preventDefault());

new ResizeObserver(() => skin?.rt.setFree(true, stage.clientWidth, stage.clientHeight)).observe(stage);

document.getElementById("skin").addEventListener("change", (e) => e.target.files[0] && open(e.target.files[0]));
document.getElementById("defaults").addEventListener("change", async (e) => {
  const file = e.target.files[0];
  if (file) defaults = new Uint8Array(await file.arrayBuffer());
});
for (const name of ["play", "pause", "stop"]) {
  document.getElementById(name).addEventListener("click", () => {
    command(name);
    skin?.invalidate();
  });
}
document.body.addEventListener("dragover", (e) => e.preventDefault());
document.body.addEventListener("drop", (e) => {
  e.preventDefault();
  const file = e.dataTransfer.files[0];
  if (file) open(file);
});
