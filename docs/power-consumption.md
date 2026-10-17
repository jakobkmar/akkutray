# Power consumption

The tray is designed to run on a laptop all day:

- It does nothing between updates: no polling loops, only a blocking Windows message loop.
- The window exists only while it is open. Closing it destroys the window and its graphics context.
- Updates run every 5 minutes on a coalescable timer, so Windows can batch the wake-up with other timers. An update
  takes a few milliseconds of CPU and some short radio messages to the devices.
