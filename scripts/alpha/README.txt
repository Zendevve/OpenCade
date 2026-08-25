OpenCade Windows Alpha Kit
==========================

This is an unsigned test build. It contains no emulator, core, BIOS, or ROM content.

1. Verify the downloaded kit:

   powershell -ExecutionPolicy Bypass -File .\OpenCade-Alpha.ps1 -Mode Verify

2. Prepare the user-supplied RetroArch layout described in RETROARCH_TEST.md.

3. Validate the machine, server, and RetroArch layout:

   powershell -ExecutionPolicy Bypass -File .\OpenCade-Alpha.ps1 -Mode Doctor `
     -ApiUrl https://alpha.example.com `
     -RetroArchRoot C:\OpenCadeAlpha\retroarch

4. Launch with the same checks and runtime configuration:

   powershell -ExecutionPolicy Bypass -File .\OpenCade-Alpha.ps1 -Mode Launch `
     -ApiUrl https://alpha.example.com `
     -RetroArchRoot C:\OpenCadeAlpha\retroarch `
     -StunServer 203.0.113.10:3478

Export host and guest reports into the reports directory. If an attempt is abandoned, export its
failure evidence so it still counts against the campaign gate. Never include emulator binaries,
ROMs, credentials, diagnostic messages, local paths, endpoints, or session material in an issue or
community post.
