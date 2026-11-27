# Smoke tests

`gpui-smoke.py` runs the GPUI player end to end against a private, silent
PulseAudio sink. It generates 440 Hz and 880 Hz tones plus one unplayable
entry, captures the sink's monitor while the player runs, and asserts that
both tones were output in queue order, the broken entry was auto-skipped,
and playback stopped after the queue ended.

```sh
dbus-run-session -- env NO_AT_BRIDGE=1 GTK_A11Y=none \
    scripts/private-audio \
    env DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$(id -u)/bus" \
    python3 scripts/gpui-smoke.py target/debug/retroamp-gpui
```

- Needs `pulseaudio` and `parec` (pulseaudio-utils), and `DISPLAY` set to a
  GPU-backed display.
- `scripts/private-audio` keeps the test off the desktop audio: PulseAudio
  must stay on the private D-Bus session, or it sees the desktop's
  `org.PulseAudio1` name and refuses to start.
- `NO_AT_BRIDGE=1 GTK_A11Y=none` keep the private session from starting its
  own accessibility bus: with dbus 1.12 a private `at-spi-bus-launcher`
  would silently replace `/run/user/$UID/at-spi/bus_0`, which the desktop
  session shares.
- The player process gets the real session bus through its own
  `DBUS_SESSION_BUS_ADDRESS` (needed for Vulkan/GPU discovery), while
  everything D-Bus-activated by the test stays private.
