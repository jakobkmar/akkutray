# Power consumption

The tray is designed to run on a laptop all day:

- It does nothing between updates: no polling loops, only a blocking Windows message loop.
- The window exists only while it is open. Closing it destroys the window and its graphics context. While it shows a
  charging device, it redraws about 30 times per second for the charging animation.
- Updates run every 5 minutes on a coalescable timer, so Windows can batch the wake-up with other timers. An update
  takes a few milliseconds of CPU and some short radio messages to the devices.
