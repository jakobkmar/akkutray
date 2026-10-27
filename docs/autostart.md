# Start at login

## Windows

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

## Linux

Desktops start the programs listed in `~/.config/autostart` at login. Add the tray there:

```sh
mkdir -p ~/.config/autostart
cat > ~/.config/autostart/akkutray.desktop <<EOF
[Desktop Entry]
Type=Application
Name=akkutray
Exec=$HOME/.cargo/bin/akkutray tray
EOF
```

**To stop starting it at login**, delete `~/.config/autostart/akkutray.desktop` again.
