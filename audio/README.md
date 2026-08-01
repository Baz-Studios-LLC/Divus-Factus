# Divus Factus Audio

## Made By Belief

`made_by_belief.mid` is a 16-bar seamless loop for the game's village/god-view state.

- Tempo: 78 BPM
- Meter: 4/4
- Mode: D Dorian
- Length: 16 bars / 64 beats
- Mood: small human settlement, sacred uncertainty, belief becoming a presence

The MIDI contains loop boundary markers named `LOOPSTART` and `LOOPEND`. If the game audio pipeline ignores MIDI markers, loop the whole file from start to end.

Files:

- `made_by_belief.mp3` - rendered full loop for quick listening
- `made_by_belief_prayer_board.mp3` - quieter drone/bell variant for prayer/Codex moments
- `made_by_belief.wav` - lossless full loop render
- `made_by_belief_prayer_board.wav` - lossless prayer/Codex render
- `made_by_belief.mid` - generated MIDI loop
- `made_by_belief.abc` - readable notation sketch
- `generate_made_by_belief.py` - dependency-free generator for the MIDI
- `render_made_by_belief.py` - dependency-free renderer for the WAV files; MP3 conversion used LAME

Arrangement notes:

- Melody: breathy reed/flute/voice-like lead
- Drone: low strings, hurdy-gurdy, bowed psaltery, or subdued synth
- Pulse: soft frame drum, muted skin, or low wooden thump
- Bell: quiet ritual accent, useful for prayer/Codex variants

For darker doctrine, transpose the melody down an octave or emphasize C natural and G. For prayer board moments, solo the drone and bell, then let the melody re-enter only when the player answers.
