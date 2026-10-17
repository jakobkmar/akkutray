# Start with Windows

To have the tray icon appear every time you log in, add a shortcut to your startup folder:

1. Press <kbd>Win</kbd>+<kbd>R</kbd>, enter `shell:startup` and press <kbd>Enter</kbd>.
2. In the folder that opens, right-click and choose *New → Shortcut*.
3. As the location, enter:

   ```
   "%USERPROFILE%\.cargo\bin\akkutray.exe" tray
   ```

   If you copied the exe somewhere else, use that path instead.
4. Name the shortcut, for example `akkutray`, and click *Finish*.

On Windows 11 24H2 and later, the tray starts silently. On older versions, a console window flashes briefly at login
before the tray moves to the background.

**To stop starting it with Windows**, delete the shortcut from the startup folder again.
