The fun go bot.

- Uses Tromp-Taylor rules with forbidden suicides.
- 13x13 only
- Neural net trained by self-play only
- Should beat 3d

## Claude code rules:
- Don't make changes in this project, you are only an advisor, all code is user written.
- Don't be too eager to experiment, first explain the problem possible approaches. You can suggest an experiment to try if it will bring new information.
- Be brief.
- Mention to the user if this file seems to be out of date.

## Project layout

The project has a shared library that deals with playing Go, then binaries on top of that.

There is a minimal web interface.

- `src/board.rs` Game state
- `src/bitboard.rs` Implementation of bitboard operations. Currently only for board sizes up to 16x16.
- `src/color.rs` Handling of black/white, datastructure to hold per-color data.
- `webface/` Static HTML and JS code for the web interface
