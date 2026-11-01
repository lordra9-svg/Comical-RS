# Windows smoke test

The Windows build is a GUI binary with no console, and no automated test can
open a window. Work through this list on a real Windows machine before
publishing a release. Every item is something that has actually been wrong or
unverified before.

Build or obtain the binary from `target/release-dist/<version>/`:

```text
comical-windows-x86_64.exe
comical_<version>-x86_64.msi      (when wixl or cargo-wix was available)
```

The log file is `%APPDATA%\comical\data\comical.log`. Delete it before the first
launch below so the file only holds this run.

## Launch

- [ ] Double-click the `.exe` from Explorer. A window opens, and **no** console
      window flashes or stays open behind it.
- [ ] Launch it from a console: `comical.exe path\to_book.cbz`. The book is on
      screen at the first page.
- [ ] Launch with no argument. The window shows the "Open a book" state.
- [ ] Close with the window button. The settings file appears at
      `%APPDATA%\comical\config\settings.toml`, and reopening restores the
      window size and position.
- [ ] Rename a book to have a wrong extension and open it. The window stays
      open and shows the open error in the canvas.

## Renderer

- [ ] Open `%APPDATA%\comical\data\comical.log`. It contains a line like:

      startup: backend=Dx12 device=DiscreteGpu adapter=<GPU name> driver=<version>

- [ ] `backend=Dx12` on a machine with a normal GPU driver. This is the check
      that the glow backend really is gone and the DX12 backend is compiled in.
- [ ] If the machine cannot provide DX12, the line may say `Vulkan` or `Gl` and
      the app still draws. Record which backend appeared and why.

## Display

- [ ] On a high-DPI screen the UI is sharp, not blurred or upscaled.
- [ ] Drag the window between a 100% and a 150% monitor. Text and the canvas
      re-scale without needing a restart, and without the window jumping size.
- [ ] Maximise, restore, and toggle F11 fullscreen. The page re-fits each time.
- [ ] Resize the window very small, then large. No panic, no blank canvas.

## Files and shell integration

- [ ] In Explorer, the `.exe` shows the book icon, not a generic blank icon.
- [ ] Right-click the `.exe`, Properties, Details: product name "Comical", a
      file version, and a description are present.
- [ ] After installing the MSI: Start menu shortcut present and working.
- [ ] Double-click a `.cbz`, `.cbr` and `.cb7`. Each opens in Comical with that
      file loaded, and the file's icon uses the installed icon.
- [ ] Non-ASCII path (for example a folder with Japanese characters) opens.
- [ ] A path longer than 260 characters opens (long-path awareness is declared
      in the manifest).

## Failure reporting

The release build cannot print, so a fatal startup error must reach the user:

- [ ] Temporarily make startup fail (for example build a local copy with an
      early `return Err("smoke test".into())` in `run`). Launching it shows a
      message box saying Comical could not start, and names the log file.
- [ ] The same run appends a `fatal:` line to `%APPDATA%\comical\data\comical.log`.
- [ ] The process exit code is 1 (`echo %ERRORLEVEL%` after running it from a
      console).

## Installer

- [ ] `msiexec /i comical_<version>-x86_64.msi` installs without a UAC prompt
      beyond the installer itself, and installs to `Program Files\Comical`.
- [ ] Uninstalling removes the program, the shortcut and the registry entries
      for the file associations, and leaves `%APPDATA%\comical` alone.
- [ ] No SmartScreen blocking beyond the expected unsigned-publisher warning.

## Record the result

Add a line to `PROVENANCE.md` with the Windows version, GPU, backend reported in
the log, and anything that failed. Do not claim a Windows release is tested
without this checklist.
