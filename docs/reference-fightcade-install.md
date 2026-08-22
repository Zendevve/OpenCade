# Repository Guidelines

## Project Overview
Fightcade 2 (v2.1.45) — online matchmaking + GGPO rollback netplay for retro arcade/console games. This directory is the **installed binary distribution** (Windows), not a source monorepo. Frontend is an Electron/Nativefier wrapper around `https://web.fightcade.com`; matchmaking launches local emulator binaries with synchronized inputs. CHangelog spans client, FcadeFBNeo (0.2.97.44), Flycast Dojo (6.46), and frontend game definitions.

## Architecture & Data Flow
- **Electron shell (`fc2-electron/`)** → Nativefier 8.0.7 app: `resources/app/lib/main.js` (webpack bundle) + `preload.js` + `lib/static/login.{html,css,js}`. `main.js` loads `https://web.fightcade.com`; internal URL `https://replay.fightcade.com`. IPC `login-message` in `login.js` (`ipcRenderer.send('login-message', [username, password])`). Config in `resources/app/nativefier.json` (`singleInstance`, `disableDevTools: true`, `width/height 768`).
- **Launcher bridge**: `Fightcade2.exe` / `Fightcade1.exe` / `fcade-upd.exe` orchestrate auth/lobby and spawn `emulator/fcade.exe` + `emulator/frm.exe` — both are **PyInstaller-frozen Python** (see `emulator/fcade-errors.log` traceback: `fightcade\launcher.py` → `fightcade\udp_client.py` → `timeout` in `start_flycast`/`init_udp`). That is where the real launcher/netcode logic lives; the `.exe` is opaque.
- **Emulator cores** (`emulator/`): `fbneo/` (arcade + FBNeo romset 0.2.97.43/44), `flycast/` (Dreamcast/NAOMI/Sega C2), `snes9x/` (SNES), `ggpofba/` (legacy). Each has `ROMs/`, `config/`, `savestates/`, `recordings/`/`replays/`, `support/`. Flycast uses `emu.cfg`/`emu.default.cfg` + `flycast_roms.json`; FBNeo uses per-system JSON catalogs.
- **ROM catalog**: `emulator/*.json` (`fbneo_roms.json` 727KB, `fbneo_*_roms.json` per system, `flycast_roms.json`, `snes9x_roms.json`, `fc1_roms.json`) enumerates supported IDs — source of truth for frontend game list and challenge sounds.
- **ROM resolution**: User-facing `ROMs/` contains only shortcuts + `README.txt`; real paths are `emulator/<core>/ROMs/<system>/` (e.g., `emulator/fbneo/ROMs/`, `emulator/flycast/ROMs/`). Missing `neogeo.zip` BIOS or wrong romset is the common failure mode.
- **Assets**: `assets/*.wav` challenge sounds (`kof98-challenge.wav` etc) + `fightcade.ico`/`icon-128.png`; shared across client notifications.

## Key Directories
- `fc2-electron/` — Electron wrapper distribution (SwiftShader, locales, `fc2-electron.exe` 91MB). Source inside `resources/app/` only.
  - `fc2-electron/resources/app/lib/` — bundled `main.js`/`preload.js` (+ `.map`) — edit is re-bundle Nativefier, not patch in place.
  - `fc2-electron/resources/app/lib/static/` — `login.html/js/css` — minimal IPC login form.
  - `fc2-electron/resources/app/inject/` — `_placeholder` — Nativefier injection hook.
- `emulator/` — native cores + configs. Do not move; `fcade.exe` expects relative `fbneo/`, `flycast/`, `snes9x/`, `ggpofba/` siblings.
  - `emulator/fbneo/` — primary arcade core (`fightcade/`, `config/`, `savestates/`, `fbneo-training-mode/`). Core binary is `emulator/fbneo/fcadefbneo.exe` (plus `fbneo/fcv39.exe`).
  - `emulator/fbneo/fbneo-training-mode/` — **only editable source surface** — vendored [peon2/fbneo-training-mode](https://github.com/peon2/fbneo-training-mode) Lua suite (~90 per-game modules). Entry `fbneo-training-mode.lua` (v0.22.10.28) + `guipages.lua`/`tableio.lua`; `games/<rom>/<rom>.lua` per game (e.g., `games/sfiii3/sfiii3.lua`, `games/kof98/kof98.lua`), `hitboxes/*.lua` profiles (`cps2-hitboxes.lua`, `garou-hitboxes.lua`), `inputs/input-display.lua`, `addon/addons.lua` registry (loads `missions.lua`). Module contract: memory addresses (`wb/ww/wdw/rb/rw/rdw`), `read/writePlayerOne*` helpers, per-frame `Run()` hook — patch here without touching the frozen launcher.
  - `emulator/flycast/` — Dreamcast/NAOMI (`flycast.exe`, `mappings/`, `data/`, `replays/`).
  - `emulator/snes9x/` — `fcadesnes9x.exe`, `fcadesnes9x.conf`, `ggponet.dll`.
  - `emulator/ggpofba/` — legacy `ggpofba-ng.exe`.
- `ROMs/` — shortcut folder only; per `ROMs/README.txt` place `.zip` in `emulator/<core>/ROMs/`.
- `assets/` — branding + per-game `*-challenge.wav` audio.

## Development Commands
This checkout has **no build/test/lint pipeline** — it is a release artifact. For AI edits:

- **Run app**: `D:/Fightcade/Fightcade2.exe` (main), `D:/Fightcade/fc2-electron/fc2-electron.exe` (wrapper alone), `D:/Fightcade/emulator/fcade.exe` (core launcher). Logs: `emulator/fcade.log` / `fcade-errors.log`.
- **Inspect Electron wrapper**: `cd fc2-electron/resources/app && npm install` (if hacking; `package.json` has no scripts, deps: `electron-context-menu@1.x`, `electron-dl@3.x`, `electron@8.x` pinned). Rebuild via Nativefier: `nativefier --targetUrl https://web.fightcade.com ...` — do not hand-edit `lib/main.js` (webpack output, 10k+ lines).
- **Emulator configs**: edit `emulator/flycast/emu.cfg`, `emulator/snes9x/fcadesnes9x.conf`, `emulator/fbneo/config/` — copy `*.default.cfg` first.
- **No npm/yarn/pnpm scripts at root**; no `tsconfig.json`, `vite.config`, or `Dockerfile` in this distribution. If adding tooling, introduce at repo root and document runtime.
- **Update**: `fcade-upd.exe` + `ChangeLog.txt` / `VERSION.txt` (current `2.1.45`).

## Code Conventions & Common Patterns
- **Wrapper**: CommonJS, `require('electron')`, `ipcRenderer` / `ipcMain` message passing (`login-message`). Nativefier bundle is webpack 4 style (`__webpack_require__`), no ESM, no TypeScript in shipped artifact.
- **Naming**: kebab for assets (`kof2002-challenge.wav`), snake-ish for emulator ROMs (`neogeo.zip`, `kof98.zip`), camelCase inside `main.js` bundle (generated).
- **Error handling**: native cores log to `fcade-errors.log` (278B example); wrapper swallows devtools (`disableDevTools: true` in `nativefier.json`). For debugging, set `disableDevTools: false` and relaunch with `--inspect`.
- **Async**: Electron IPC event-driven; emulator netcode is C++ GGPO via `ggponet.dll`/`kailleraclient.dll` — not JS async.
- **Config over code**: behavior driven by JSON catalogs (`emulator/*.json`) and `nativefier.json`; prefer data edits over code patches.
- **State**: no Redux/Zustand in shipped wrapper — server-authoritative lobby at `web.fightcade.com`; local state is filesystem (ROM presence, `savestates/`, `recordings/`).

## Important Files
- Entry: `Fightcade2.exe`, `Fightcade1.exe` — top-level launchers.
- Electron: `fc2-electron/resources/app/package.json`, `fc2-electron/resources/app/nativefier.json`, `fc2-electron/resources/app/lib/main.js`, `fc2-electron/resources/app/lib/preload.js`, `fc2-electron/resources/app/lib/static/login.js`
- Emulators: `emulator/fcade.exe`, `emulator/frm.exe`, `emulator/fbneo/fcadefbneo.exe` (core; also `emulator/fbneo/fcv39.exe`), `emulator/flycast/flycast.exe`, `emulator/snes9x/fcadesnes9x.exe`
- Catalogs: `emulator/fbneo_roms.json`, `emulator/fbneo_{sms,nes,md,cv,gg,msx,pce,sg1k,tg}_roms.json`, `emulator/flycast_roms.json`, `emulator/snes9x_roms.json`
- Docs/state: `VERSION.txt`, `ChangeLog.txt` (1078 lines), `ROMs/README.txt`, `assets/fightcade.ico`
- Logs/config: `emulator/fcade.log`, `emulator/fcade-errors.log`, `emulator/flycast/emu.cfg`, `emulator/snes9x/fcadesnes9x.conf`

## Runtime/Tooling Preferences
- **OS**: Windows 10 IoT Enterprise LTSC 2021 (win32 10.0.19044, x64, Intel R Core i3-8145U). Paths are `D:/Fightcade` — use Windows separators; forwarding slashes work in Node but not in `.lnk` shortcuts.
- **Electron**: 8.x (bundled); Nativefier 8.0.7. If modifying wrapper, pin `electron@8.x` as in `resources/app/package.json`; newer Electron breaks native modules.
- **Package manager**: none at root (distribution). Inside `fc2-electron/resources/app` use `npm` (no lockfile shipped). Prefer `npm ci` if you add one.
- **No Bun/Node version enforced** — wrapper uses bundled Chromium/Node; emulators are native `.exe`/`.dll` (`d3dcompiler_47.dll`, `ffmpeg.dll`, `libEGL.dll`, etc. in `fc2-electron/`).
- **Tooling constraint**: do not add TypeScript/bundler without a build step that re-emits `resources/app/lib/main.js` + source map; keep `disableContextMenu`/`disableDevTools` intentional for production.

## Testing & QA
- **No automated test suite** in this distribution (`jest`/`vitest`/`playwright` configs absent; no `__tests__/`).
- **Manual QA** (expected):
  - Launch `Fightcade2.exe` → login via `login.html` form → lobby appears (server `web.fightcade.com`).
  - ROM check: place `kof98.zip` + `neogeo.zip` in `emulator/fbneo/ROMs/`; launch from lobby — expect no `xxxx.zip was not found` / incompatibility error (requires romset 0.2.97.44; FC1 roms fail on FC2).
  - Emulator smoke: `emulator/fbneo` vs `emulator/flycast` vs `emulator/snes9x` each launch, inputs respond, `fightcade/` replays/spectate work (see ChangeLog 2.1.45: session transmission fix).
  - Challenging sound: trigger challenge → `assets/*-challenge.wav` plays.
- **Logs for verification**: `emulator/fcade-errors.log`, `emulator/fcade.log`, Flycast `emu.cfg.bak` diff after config change.
- **If adding tests** for wrapper changes: use `vitest` or `jest` + `electron-mock-ipc`; place in `tests/` or `__tests__/` at root, mock `ipcRenderer`, and add `npm test` script — keep emulator binaries out of CI (large, Windows-only).
