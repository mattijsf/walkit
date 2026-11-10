// SPDX-License-Identifier: GPL-3.0-or-later
//
// The C interface of the walkit-macos library (rust/src/lib.rs): a skin engine with its host,
// a demo player and a software renderer. Every call runs on the app's main thread.

#ifndef WALKIT_MACOS_H
#define WALKIT_MACOS_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/// A loaded skin.
typedef struct WalkitSkin WalkitSkin;

/// Opens the skin archive (.wsz or .wal) of `skin_len` bytes at `skin`, named `name` (its
/// file name). `base` is a classic skin laid under a classic skin, `wasabi` a zip of Winamp's
/// wasabi folder laid under a modern one; either may be null. `fonts_dir` is a folder of the
/// stand-in fonts, or null. On failure it gives null and, when `error` is not null, a message
/// in `*error` to free with walkit_string_free.
WalkitSkin *walkit_skin_open(const uint8_t *skin, size_t skin_len, const char *name,
                             const uint8_t *base, size_t base_len, const uint8_t *wasabi,
                             size_t wasabi_len, const char *fonts_dir, char **error);

/// Frees a skin walkit_skin_open gave.
void walkit_skin_free(WalkitSkin *skin);

/// Frees a message walkit_skin_open gave.
void walkit_string_free(char *text);

/// Runs the engine's timers and animations; true when the skin wants a new frame.
bool walkit_skin_tick(WalkitSkin *skin);

/// The size the skin's windows take, in layout pixels.
void walkit_skin_size(WalkitSkin *skin, int32_t *w, int32_t *h);

/// Draws a frame of `w` x `h` device pixels, `scale` device pixels per layout pixel, and gives
/// its `w * h` pixels, rows top first, each 0x00RRGGBB. They stay valid until the next draw or
/// until the skin is freed; null when the skin broke.
const uint32_t *walkit_skin_draw(WalkitSkin *skin, int32_t w, int32_t h, int32_t scale);

/// The mouse, in layout pixels: down (`touch` numbers the press), moved while down, and up.
void walkit_skin_pointer_down(WalkitSkin *skin, int32_t x, int32_t y, int32_t touch);
void walkit_skin_pointer_move(WalkitSkin *skin, int32_t x, int32_t y);
void walkit_skin_pointer_up(WalkitSkin *skin, int32_t x, int32_t y);

/// A double click; `touch` numbers its press.
void walkit_skin_double_click(WalkitSkin *skin, int32_t x, int32_t y, int32_t touch);

#endif
