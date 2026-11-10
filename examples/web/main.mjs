// SPDX-License-Identifier: GPL-3.0-or-later
// A page that shows a Winamp skin with walkit's embedding API: one canvas per window, a small
// player of its own, and the pointer passed to the engine.
import { loadWalkit } from "./walkit-web/embed.mjs";

// Winamp's own skins, from the Winamp Skin Museum. When a download fails, the page shows the
// original skin of the same kind that ships beside the examples. Winamp's base skin also lies
// under every classic skin: it gives the bitmaps a skin leaves out.
const SKINS = {
  classic: {
    url: "https://r2.webampskins.org/skins/5e4f10275dcb1fb211d4a8b4f1bda236.wsz",
    name: "Winamp Base.wsz",
    fallback: "../skins/walkit-classic.wsz",
  },
  modern: {
    url: "https://r2.webampskins.org/skins/b3fa3160814d1689a3f2bacda72b2926.wal",
    name: "Winamp Modern.wal",
    fallback: "../skins/walkit-modern.wal",
  },
};

// The free fonts that stand in for Windows' Arial and Tahoma, as the engine names them.
const FONTS = ["LiberationSans.ttf", "LiberationSans-Bold.ttf", "DejaVuSansCondensed.ttf", "DejaVuSansCondensed-Bold.ttf"];

const stage = document.getElementById("stage");
const hint = document.getElementById("hint");

// The page's player: a fixed playlist and a clock. The engine only shows what it is told.
const tracks = [
  { title: "walkit - Example track", lengthMs: 215000 },
  { title: "walkit - Another track", lengthMs: 184000 },
];
const player = {
  status: "stopped",
  index: 0,
  startedAt: 0,
  positionMs: 0,
  volume: 200,
  pan: 0,
  shuffle: false,
  repeat: false,
  eqOn: false,
  preamp: 128,
  bands: new Array(10).fill(128),
};

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
    case "pan": player.pan = value; break;
    case "shuffle": player.shuffle = value !== 0; break;
    case "repeat": player.repeat = value !== 0; break;
    case "eq": player.eqOn = value !== 0; break;
    case "preamp": player.preamp = value; break;
    default:
      if (name.startsWith("band")) player.bands[Number(name.slice(4))] = value;
  }
}

let skin = null;
let wasabi = null;
const canvases = new Map(); // window id -> canvas

const walkit = await loadWalkit({
  invalidate: () => skin?.invalidate(),
  player: (name, value) => command(name, value),
  action: (action, param) => console.log("action", action, param),
  log: (line) => console.log(line),
});
await walkit.addFonts(
  new Map(
    await Promise.all(
      FONTS.map(async (name) => [`fonts/${name}`, new Uint8Array(await (await fetch(`../skins/fonts/${name}`)).arrayBuffer())]),
    ),
  ),
);

/** Downloads skin `kind` ("classic" or "modern"), or reads its fallback when that fails. */
async function fetchSkin(kind) {
  const skin = SKINS[kind];
  try {
    const response = await fetch(skin.url, { signal: AbortSignal.timeout(10000) });
    if (!response.ok) throw new Error(`${response.status}`);
    return new File([await response.blob()], skin.name);
  } catch (e) {
    console.log(`${skin.url} failed (${e}); showing ${skin.fallback}`);
    const response = await fetch(skin.fallback);
    return new File([await response.blob()], skin.fallback.split("/").pop());
  }
}

// Winamp's base skin (or the built-in classic skin when the download fails), fetched once: the
// same bytes each time, so the engine opens it once
let classicBase = null;
function baseSkin() {
  classicBase ??= fetchSkin("classic").then(async (file) => ({ file, bytes: new Uint8Array(await file.arrayBuffer()) }));
  return classicBase;
}

async function open(file) {
  skin?.free();
  for (const canvas of canvases.values()) canvas.remove();
  canvases.clear();
  const bytes = new Uint8Array(await file.arrayBuffer());
  // a classic skin lies over Winamp's base skin, a modern one over the wasabi base
  const base = (await baseSkin()).bytes;
  skin = await walkit.openSkin(bytes, { name: file.name.replace(/\.\w+$/, ""), base, wasabi });
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
      pan: player.pan,
      shuffle: player.shuffle,
      repeat: player.repeat,
      eqOn: player.eqOn,
      preamp: player.preamp,
      bands: player.bands,
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
document.getElementById("wasabi").addEventListener("change", async (e) => {
  const file = e.target.files[0];
  if (file) wasabi = new Uint8Array(await file.arrayBuffer());
});
document.getElementById("classic").addEventListener("click", async () => open((await baseSkin()).file));
document.getElementById("modern").addEventListener("click", async () => open(await fetchSkin("modern")));
for (const kind of Object.keys(SKINS)) {
  // the original skin of the same kind, without trying the download
  document.getElementById(`walkit-${kind}`).addEventListener("click", async () => {
    const path = SKINS[kind].fallback;
    open(new File([await (await fetch(path)).blob()], path.split("/").pop()));
  });
}
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

open((await baseSkin()).file);
