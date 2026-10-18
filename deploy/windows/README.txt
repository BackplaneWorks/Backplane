Backplane for Windows x64

Extract the entire zip into a writable directory.
If Cygwin is not already installed at C:\cygwin64, right-click
Install-Runtime.ps1 and choose Run with PowerShell. This downloads the
official Cygwin installer and installs the runtime under your Windows
user account; administrator access is not required.

Open Backplane.cmd to start the native Windows window. A different Cygwin
installation can be selected with BACKPLANE_CYGWIN_ROOT. Keep web/ beside
backplane.exe. Provider accounts are configured separately; this archive
contains no credentials. No WSL is required.

This initial target requires Cygwin's shell, files, sockets and font
libraries. Optional browser/KiCad helpers require separate Windows builds.
Custom IME preedit and clipboard images are not implemented yet.
The runtime and provider applications retain their own licenses.
