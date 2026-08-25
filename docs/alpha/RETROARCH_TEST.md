# RetroArch Proof-of-Play alpha

This procedure tests the experimental native-process adapter from ADR 0003. It requires two real
Windows machines and user-supplied emulator files. Never commit, upload, or attach emulator binaries,
cores, BIOS files, or ROMs to OpenCade reports.

## Get the alpha artifact

Download `opencade-windows-alpha` from a successful `main` CI run. Verify every file against
`SHA256SUMS.txt` before running it. The artifact is a flat directory; run the same verifier used by
CI from inside it:

```powershell
powershell -ExecutionPolicy Bypass -File .\OpenCade-Alpha.ps1 -Mode Verify
```

The artifact is an unsigned alpha executable, not a production installer. Windows may display a
reputation warning. Production signing remains a separate M7 gate.

## Prepare RetroArch on both machines

Create the same directory layout on each machine:

```text
C:\OpenCadeAlpha\retroarch\
├── retroarch.exe
├── VERSION.txt                 # exact RetroArch version, one line
├── cores\
│   └── fbneo_libretro.dll
└── ROMs\
    ├── neogeo.zip              # when required by the selected game
    └── sfiii3.zip              # user-supplied content
```

Use the same RetroArch version, FBNeo core, and exact content on both machines. OpenCade computes
SHA-256 fingerprints locally and includes only those hashes—not paths or content—in an exported
report.

## Start the clients

On each machine, open PowerShell in the alpha artifact directory. The alpha doctor verifies the
artifact, control-plane health/readiness, optional STUN syntax, and required RetroArch layout before
starting the client:

```powershell
powershell -ExecutionPolicy Bypass -File .\OpenCade-Alpha.ps1 -Mode Launch `
  -ApiUrl "https://alpha.example.com" `
  -RetroArchRoot "C:\OpenCadeAlpha\retroarch" `
  -StunServer "203.0.113.10:3478"
```

Remote control-plane URLs must use HTTPS. Loopback development may use
`http://127.0.0.1:8080`. Allow the server TCP port, the OpenCade probe UDP ports, and RetroArch TCP
port `55435` through the appropriate firewalls. Do not expose a developer laptop directly to the
public internet.

## Scenario

1. Register two different OpenCade users and select the same game.
2. Challenge and accept until both clients reach the match screen.
3. Wait for `Direct UDP verified` with a same-LAN host candidate and matching 60-frame transcript
   checksums. Relay and reflexive routes are readiness-only and keep native launch disabled.
4. Select **Launch playable alpha** on the host first, then on the guest.
5. Confirm both clients display the same executable, core, and content hash prefixes.
6. Confirm RetroArch establishes netplay and player two can provide gameplay input.
7. End the session cleanly. Export one report from each OpenCade client after the native process has
   launched so the compatibility fingerprints are included.
8. Run `opencade-match-verify --require-compatibility host.json guest.json`. Missing or mismatched
   adapter fingerprints, transport, transcript, room, game, or roles must fail closed.
9. If RetroArch launch fails, export failure evidence before leaving the match screen. The report
   records `native_launch_failed`, but never the local launch error or path.

## Pass criteria

- Both OpenCade readiness reports verify as a pair.
- Both reports contain identical native-process compatibility fingerprints.
- The host and guest processes were launched using the documented process boundary without a shell.
- RetroArch reports a connected netplay session and player-two input is observed.
- No report contains a local path, endpoint, user identifier, session token, ROM name, or binary.

Record manual gameplay observations in `MATCH_REPORT_TEMPLATE.md`. Do not mark RetroArch netplay as
proven until at least one same-LAN Proof-of-Play passes; do not mark internet play as proven until the
five different-NAT campaign attempts meet the aggregate gate.
