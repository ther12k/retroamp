"""End-to-end audio check for the GPUI player (no display or audio hardware assumptions beyond a working Vulkan surface).

Generates 440 Hz and 880 Hz tones plus one unplayable entry, runs the player
with them as the queue, captures the private sink's monitor, and asserts the
tones were output in queue order, the broken entry was skipped, and playback
stopped after the queue ended.

Usage: scripts/private-audio python3 scripts/gpui-smoke.py target/debug/retroamp-gpui
"""

import array
import math
import pathlib
import subprocess
import sys
import tempfile
import time
import wave

exe = pathlib.Path(sys.argv[1]).resolve(strict=True)
root = pathlib.Path(tempfile.mkdtemp(prefix='retroamp-smoke-'))
rate = 44100
paths = []
for freq in (440, 880):
    path = root / f'{freq} Hz.wav'
    paths.append(path)
    with wave.open(str(path), 'wb') as wav:
        wav.setparams((2, 2, rate, 0, 'NONE', 'not compressed'))
        data = array.array('h')
        for i in range(3 * rate):
            sample = int(8192 * math.sin(2 * math.pi * freq * i / rate))
            data.extend((sample, sample))
        wav.writeframes(data.tobytes())
paths.insert(1, root / 'unreadable-track.wav')  # auto-advance must skip this broken file


def stop(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


with (root / 'monitor.raw').open('wb') as output:
    recorder = subprocess.Popen(
        ['parec', '--device=retroamp_gpui_test.monitor', '--format=s16le', f'--rate={rate}',
         '--channels=1', '--latency-msec=50', '--raw'], stdout=output)
    try:
        app = subprocess.Popen([exe, *paths])
        try:
            for _ in range(60):
                time.sleep(0.20)
                if app.poll() is not None:
                    raise RuntimeError(f'player exited during the smoke run: {app.returncode}')
        finally:
            stop(app)
    finally:
        stop(recorder)

samples = array.array('h', (root / 'monitor.raw').read_bytes())
step = rate // 10
refs = [[(math.cos(2 * math.pi * freq * j / rate), math.sin(2 * math.pi * freq * j / rate))
         for j in range(step)] for freq in (440, 880)]
sequence = []
for offset in range(0, len(samples) - step, step):
    block = samples[offset:offset + step]
    amps = [abs(complex(sum(x * z[0] for x, z in zip(block, ref)),
                        sum(x * z[1] for x, z in zip(block, ref)))) * 2 / step for ref in refs]
    freq = (440, 880)[int(amps[1] > amps[0])] if max(amps) > 500 else 0
    if not sequence or sequence[-1] != freq:
        sequence.append(freq)
print(f'Captured {len(samples) / rate:.2f} s on the private sink monitor; '
      f'dominant frequencies (0 = silence): {sequence}')
sounds = [freq for freq in sequence if freq]
assert sounds and sounds[0] == 440 and 880 in sounds and sounds[-1] == 880 and sequence[-1] == 0, sequence
print('PASS: the window stayed alive, both tones were output in queue order with the broken entry skipped, then playback stopped.')
