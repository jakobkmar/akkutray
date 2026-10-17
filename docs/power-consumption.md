# Power consumption

The tray is built to run on a laptop all day without draining it.

- **It sleeps between updates.** There is no polling loop: the tray waits in a blocking Windows message loop until
  something happens.
- **Updates are short and rare.** Every 5 minutes, an update takes a few milliseconds of CPU and a few short radio
  messages to the devices. The timer is coalescable, so Windows can bundle the wake-up with other timers.
- **The window only exists while it is open.** Closing it destroys the window and its graphics context. While it
  shows a charging device, it redraws about 30 times per second for the charging animation.
