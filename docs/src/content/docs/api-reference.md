---
title: API reference
---

The Rust API reference is built by rustdoc from the engine's doc comments:

- [`walkit` on docs.rs](https://docs.rs/walkit): the engine as released on crates.io, one
  reference per version.
- [`walkit` here](/walkit/api/walkit/index.html): the engine on `main`, which can be ahead of
  the last release.

Start at [`SkinRuntime`](/walkit/api/walkit/runtime/struct.SkinRuntime.html), the
[`SkinHost`](/walkit/api/walkit/host/trait.SkinHost.html) and
[`Player`](/walkit/api/walkit/player/trait.Player.html) traits, and
[`DrawCmd`](/walkit/api/walkit/draw/enum.DrawCmd.html).

The web package's types are in
[`web/embed.d.ts`](https://github.com/mattijsf/walkit/blob/main/web/embed.d.ts).
