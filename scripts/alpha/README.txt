OpenCade Windows Alpha Kit
==========================

This is an unsigned test build. It contains no emulator, BIOS, or ROM content. It includes one
original Apache-2.0 deterministic libretro test core and harmless OpenCade test content.

1. Verify the downloaded kit:

   powershell -ExecutionPolicy Bypass -File .\OpenCade-Alpha.ps1 -Mode Verify

2. Prepare user-supplied RetroArch, then explicitly install the verified no-ROM test fixture:

   powershell -ExecutionPolicy Bypass -File .\OpenCade-Alpha.ps1 -Mode InstallTestFixture `
     -RetroArchRoot C:\OpenCadeAlpha\retroarch

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
