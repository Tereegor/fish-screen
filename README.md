# fish-screen

A rotating fish for your Linux terminal. The binary includes the animation.

```sh
cargo install --locked --path .
fish-screen
fish-screen --mode original --fg '#ffbf69' --bg '#101418' --size 65 --speed 0.75
```

ASCII is the default. A direct Kitty session also supports pixel mode. Press `g` to switch without restarting the loop. Press `q`, Escape or Ctrl+C to quit.

Text modes blend adjacent frames at up to 60 FPS. Set the target with `--fps 1..120`. Pixel mode plays the GIF's actual frames. Use `--mode blocks` for a shaded block silhouette or `--glyph-size 1..4` to start with larger text. Large ASCII letters use Unicode Braille dots; the terminal font size stays unchanged.

| Keys | Action |
| --- | --- |
| Ctrl + plus / minus | Enlarge / shrink symbols without resizing the fish |
| Ctrl + 1 / 2 | Speed up / slow down by a factor of 1.25 |
| Ctrl + 3 / 4 / 5 | ASCII / blocks / pixels (ASCII outside Kitty) |
| Space | Pause / resume |

If your terminal intercepts Ctrl shortcuts, use the same keys without Ctrl. The equals key also enlarges symbols.

The fish fits the window using the terminal's reported cell dimensions, with a 1:2 fallback.

Use `--size 1..100` (default 80 percent) and `--speed 0.25..4` (default 1×). Both modes use the same foreground and background colors, white on black by default. Choose black, red, green, yellow, blue, magenta, cyan, white or a quoted hex color. Pass `--gif PATH` to play another GIF.
