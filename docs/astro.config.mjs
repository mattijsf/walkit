// SPDX-License-Identifier: GPL-3.0-or-later
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

export default defineConfig({
  site: "https://mattijsf.github.io",
  base: "/walkit",
  integrations: [
    starlight({
      title: "walkit",
      description: "A Winamp skin engine, as a library.",
      social: [{ icon: "github", label: "GitHub", href: "https://github.com/mattijsf/walkit" }],
      editLink: { baseUrl: "https://github.com/mattijsf/walkit/edit/main/docs/" },
      sidebar: [
        { label: "Start", items: ["introduction", "how-it-works", "examples"] },
        { label: "Hosts", items: ["rust", "web", "android"] },
        { label: "Reference", items: ["draw-list", "api-reference", { label: "Rust API (rustdoc)", link: "/api/walkit/" }] },
        { label: "Engine", items: ["traces", "contributing"] },
      ],
    }),
  ],
});
