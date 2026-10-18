Backplane for Windows x64

Extract the entire zip into a writable directory.
If Cygwin is not already installed at C:\cygwin64, right-click
Install-Runtime.ps1 and choose Run with PowerShell. This downloads the
official Cygwin installer and installs the runtime under your Windows
user account; administrator access is not required.

Open Backplane.cmd to start the native Windows window. A different Cygwin
installation can be selected with BACKPLANE_CYGWIN_ROOT. Keep web/ beside
backplane.exe. Install-Providers.ps1 installs missing native Claude/Codex CLIs. Sign in
with your provider yourself. Provider accounts are configured separately; this archive
contains no credentials. No WSL is required.

This initial target requires Cygwin's shell, files, sockets and font
libraries. The embedded browser still requires its own Windows helper build.
Custom IME preedit and clipboard images are not implemented yet.
The runtime and provider applications retain their own licenses.

CAD helpers are included beside backplane.exe. Keep the licenses directory.
Install FreeCAD separately and set BACKPLANE_FREECAD to its actual
bin/freecadcmd.exe before starting Backplane. Portable bundles can contain
asynchronous root-level shims; the STEP helper uses the real bin GUI.
Set BACKPLANE_KICAD to the Backplane KiCad runtime root or bin/kicad-cli.exe.
Stock KiCad lacks some IPC capabilities; the runtime health check stays active.
