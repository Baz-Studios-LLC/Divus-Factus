"""Synthesize Divus Factus's sound kit from pure math - warm, soft, low-poly
sounds to match a low-poly world. 22050 Hz mono 16-bit WAV."""
import wave, math, random, struct, os

SR = 22050
random.seed(0x1CE)

def write(name, samples):
    peak = max(1e-9, max(abs(s) for s in samples))
    norm = 0.92 / peak if peak > 0.92 else 1.0
    path = os.path.join("assets/audio/sfx", name)
    with wave.open(path, "w") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR)
        w.writeframes(b"".join(struct.pack("<h", int(max(-1, min(1, s * norm)) * 32767)) for s in samples))
    print(f"{name:14} {len(samples)/SR:.2f}s")

def silence(t): return [0.0] * int(SR * t)

def mix(base, add, at=0.0):
    off = int(SR * at)
    need = off + len(add) - len(base)
    if need > 0: base.extend([0.0] * need)
    for i, s in enumerate(add): base[off + i] += s
    return base

def lowpass(xs, alpha):
    y, out = 0.0, []
    for x in xs:
        y += alpha * (x - y); out.append(y)
    return out

def env_exp(n, decay):
    return [math.exp(-decay * i / SR) for i in range(n)]

def tone(freq, t, decay, amp=1.0, bend=0.0):
    n = int(SR * t); out = []
    phase = 0.0
    for i in range(n):
        f = freq * (1.0 + bend * i / n)
        phase += 2 * math.pi * f / SR
        out.append(amp * math.sin(phase) * math.exp(-decay * i / SR))
    return out

def noise(t, decay, amp=1.0):
    n = int(SR * t)
    return [amp * (random.random() * 2 - 1) * math.exp(-decay * i / SR) for i in range(n)]

# The knock: two knuckle raps on wood - a thump with a woody snap, twice,
# spaced like the hand's own two strikes.
def rap():
    body = tone(95, 0.09, 45, 1.0, -0.25)
    snap = lowpass(noise(0.02, 160, 0.8), 0.5)
    return mix(body, snap)
knock = rap()
knock = mix(knock, [s * 0.85 for s in rap()], 0.19)
write("knock.wav", knock)

# The splash: a bloop (pitch falling under water) inside a soft wash of foam.
bloop = tone(420, 0.22, 18, 0.9, -0.55)
foam = lowpass(noise(0.5, 9, 0.8), 0.18)
splash = mix(foam, bloop, 0.02)
write("splash.wav", splash)

# The thud: something heavy meeting the turf.
thud = tone(70, 0.16, 30, 1.0, -0.2)
thud = mix(thud, lowpass(noise(0.03, 120, 0.5), 0.35))
write("thud.wav", thud)

# The grab: a quick soft pluck - the hand closing on something.
grab = tone(240, 0.08, 55, 0.6, 0.35)
grab = mix(grab, lowpass(noise(0.05, 70, 0.35), 0.3))
write("grab.wav", grab)

# The whoosh: air over knuckles as a thing is hurled.
n = int(SR * 0.28)
raw = [(random.random() * 2 - 1) for _ in range(n)]
sweep = []
y = 0.0
for i, x in enumerate(raw):
    a = 0.08 + 0.5 * math.sin(math.pi * i / n)   # filter opens then closes
    y += a * (x - y)
    swell = math.sin(math.pi * i / n) ** 2
    sweep.append(y * swell)
write("whoosh.wav", sweep)

# The smite: a crack of thunder and the rumble that owns the valley after.
crack = noise(0.06, 40, 1.0)
rumble_raw = noise(2.2, 1.6, 1.0)
brown, y = [], 0.0
for x in rumble_raw:
    y = max(-1, min(1, y + x * 0.14)); brown.append(y)
rumble = lowpass(brown, 0.045)
smite = mix([s * 1.1 for s in crack], [r * 0.9 for r in rumble], 0.03)
smite = mix(smite, tone(55, 1.4, 2.4, 0.5), 0.05)
write("smite.wav", smite)

# The chime: the prayer bell - soft inharmonic partials, the pink channel's voice.
chime = tone(880, 1.3, 3.2, 0.5)
chime = mix(chime, tone(880 * 2.76, 0.9, 5.5, 0.22))
chime = mix(chime, tone(880 * 0.5, 1.5, 2.4, 0.3))
write("chime.wav", chime)

# The fanfare: two warm strikes rising - a village remembering a good day.
fan = tone(660, 0.9, 3.4, 0.5)
fan = mix(fan, tone(660 * 1.5, 1.1, 3.0, 0.45), 0.16)
fan = mix(fan, tone(660 * 2.0, 0.8, 4.4, 0.2), 0.16)
write("fanfare.wav", fan)

# The planting: the flag driven home - a firm thump and a small shimmer of
# something beginning.
plant = tone(85, 0.2, 26, 1.0, -0.15)
plant = mix(plant, lowpass(noise(0.04, 100, 0.5), 0.4))
plant = mix(plant, tone(1320, 0.7, 6.0, 0.16), 0.1)
plant = mix(plant, tone(1980, 0.5, 8.0, 0.1), 0.16)
write("plant.wav", plant)
