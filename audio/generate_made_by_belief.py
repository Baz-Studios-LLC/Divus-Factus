#!/usr/bin/env python3
"""Generate the Made By Belief MIDI loop.

Dependency-free on purpose: this can run anywhere Python is available.
"""

from __future__ import annotations

from pathlib import Path
from typing import Iterable

PPQ = 480
BEAT = PPQ
BAR = BEAT * 4
EIGHTH = BEAT // 2
TEMPO_BPM = 78
MICROSECONDS_PER_BEAT = round(60_000_000 / TEMPO_BPM)

OUT = Path(__file__).with_name("made_by_belief.mid")


def varlen(value: int) -> bytes:
    if value < 0:
        raise ValueError("negative delta")
    buffer = value & 0x7F
    out = []
    while value := value >> 7:
        buffer <<= 8
        buffer |= (value & 0x7F) | 0x80
    while True:
        out.append(buffer & 0xFF)
        if buffer & 0x80:
            buffer >>= 8
        else:
            break
    return bytes(out)


def chunk(kind: bytes, data: bytes) -> bytes:
    return kind + len(data).to_bytes(4, "big") + data


def meta(delta: int, kind: int, data: bytes) -> bytes:
    return varlen(delta) + bytes([0xFF, kind]) + varlen(len(data)) + data


def program(delta: int, channel: int, program_number: int) -> bytes:
    return varlen(delta) + bytes([0xC0 | channel, program_number])


def note_on(delta: int, channel: int, note: int, velocity: int) -> bytes:
    return varlen(delta) + bytes([0x90 | channel, note, velocity])


def note_off(delta: int, channel: int, note: int) -> bytes:
    return varlen(delta) + bytes([0x80 | channel, note, 0])


def controller(delta: int, channel: int, cc: int, value: int) -> bytes:
    return varlen(delta) + bytes([0xB0 | channel, cc, value])


def pitch(name: str) -> int:
    names = {
        "C": 0,
        "C#": 1,
        "D": 2,
        "Eb": 3,
        "E": 4,
        "F": 5,
        "F#": 6,
        "G": 7,
        "Ab": 8,
        "A": 9,
        "Bb": 10,
        "B": 11,
    }
    note = name[:-1]
    octave = int(name[-1])
    return 12 * (octave + 1) + names[note]


def notes_track(
    name: str,
    channel: int,
    program_number: int,
    notes: Iterable[tuple[int, int, int, int]],
    volume: int = 90,
    pan: int = 64,
) -> bytes:
    events: list[tuple[int, int, bytes]] = []
    for start, duration, note, velocity in notes:
        events.append((start, 1, bytes([0x90 | channel, note, velocity])))
        events.append((start + duration, 0, bytes([0x80 | channel, note, 0])))
    events.sort(key=lambda e: (e[0], e[1]))

    data = bytearray()
    data += meta(0, 0x03, name.encode("ascii"))
    data += program(0, channel, program_number)
    data += controller(0, channel, 7, volume)
    data += controller(0, channel, 10, pan)
    last = 0
    for time, _, payload in events:
        data += varlen(time - last) + payload
        last = time
    data += meta(max(BAR * 16 - last, 0), 0x2F, b"")
    return chunk(b"MTrk", bytes(data))


def tempo_track() -> bytes:
    data = bytearray()
    data += meta(0, 0x03, b"Made By Belief")
    data += meta(0, 0x51, MICROSECONDS_PER_BEAT.to_bytes(3, "big"))
    data += meta(0, 0x58, bytes([4, 2, 24, 8]))
    data += meta(0, 0x06, b"LOOPSTART")
    data += meta(BAR * 16, 0x06, b"LOOPEND")
    data += meta(0, 0x2F, b"")
    return chunk(b"MTrk", bytes(data))


def melody_notes() -> list[tuple[int, int, int, int]]:
    bars = [
        ["D4", "F4", "E4", "D4"],
        ["A3", "D4", "F4", "E4"],
        ["G4", "F4", "E4", "C4"],
        ["D4", None, "A3", None],
        ["D4", "F4", "A4", "G4"],
        ["F4", "E4", "D4", "C4"],
        ["A3", "C4", "D4", "F4"],
        ["E4", None, "D4", None],
        ["F4", "A4", "G4", "F4"],
        ["E4", "D4", "C4", "A3"],
        ["C4", "D4", "F4", "E4"],
        ["D4", None, "A3", None],
        ["G4", "F4", "E4", "D4"],
        ["A3", "D4", "C4", "A3"],
        ["D4", "F4", "E4", "C4"],
        ["D4", None, None, None],
    ]
    out = []
    for bar_index, notes in enumerate(bars):
        start = bar_index * BAR
        for i, note in enumerate(notes):
            if note is None:
                continue
            duration = BEAT * (4 if bar_index == 15 else 1)
            if i == 0 and bar_index in {3, 7, 11}:
                duration = BEAT * 2
            out.append((start + i * BEAT, duration, pitch(note), 72))
    return out


def drone_notes() -> list[tuple[int, int, int, int]]:
    roots = [
        ("D2", "A1"),
        ("D2", "A1"),
        ("C2", "G1"),
        ("D2", "A1"),
        ("D2", "A1"),
        ("C2", "G1"),
        ("A1", "C2"),
        ("D2", "A1"),
        ("D2", "A1"),
        ("C2", "G1"),
        ("A1", "C2"),
        ("D2", "A1"),
        ("G1", "D2"),
        ("A1", "C2"),
        ("D2", "A1"),
        ("D2", "A1"),
    ]
    out = []
    for bar_index, chord in enumerate(roots):
        start = bar_index * BAR
        for note in chord:
            out.append((start, BAR, pitch(note), 46))
    return out


def pulse_notes() -> list[tuple[int, int, int, int]]:
    out = []
    for bar_index in range(16):
        start = bar_index * BAR
        for beat_index, velocity in [(0, 58), (2, 45)]:
            out.append((start + beat_index * BEAT, EIGHTH // 2, 45, velocity))
        if bar_index in {3, 7, 11, 15}:
            out.append((start + 3 * BEAT, EIGHTH // 2, 60, 34))
    return out


def bell_notes() -> list[tuple[int, int, int, int]]:
    out = []
    for bar_index in [0, 8, 15]:
        start = bar_index * BAR
        out.append((start, BEAT * 2, pitch("D5"), 42))
        out.append((start + EIGHTH, BEAT, pitch("A4"), 34))
    return out


def main() -> None:
    header = chunk(b"MThd", bytes([0, 1, 0, 5]) + PPQ.to_bytes(2, "big"))
    tracks = [
        tempo_track(),
        notes_track("Melody - reed voice", 0, 75, melody_notes(), volume=88, pan=68),
        notes_track("Drone - low bowed strings", 1, 42, drone_notes(), volume=70, pan=42),
        notes_track("Pulse - soft frame drum", 9, 0, pulse_notes(), volume=78, pan=64),
        notes_track("Prayer bell", 2, 14, bell_notes(), volume=48, pan=58),
    ]
    OUT.write_bytes(header + b"".join(tracks))
    print(OUT)


if __name__ == "__main__":
    main()

