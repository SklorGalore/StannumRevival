// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";

// Published to GitHub Pages at https://sklorgalore.github.io/StannumRevival/.
export default defineConfig({
  site: "https://sklorgalore.github.io",
  base: "/StannumRevival",
  markdown: {
    remarkPlugins: [remarkMath],
    rehypePlugins: [rehypeKatex],
  },
  integrations: [
    starlight({
      title: "Stannum",
      description: "Documentation for Stannum, a desktop workbench for power system engineers.",
      logo: { src: "./src/assets/logo-small.svg" },
      favicon: "/favicon.svg",
      social: [{ icon: "github", label: "GitHub", href: "https://github.com/SklorGalore/StannumRevival" }],
      editLink: { baseUrl: "https://github.com/SklorGalore/StannumRevival/edit/main/docs-site/" },
      customCss: ["katex/dist/katex.min.css", "./src/styles/stannum.css"],
      lastUpdated: true,
      sidebar: [
        {
          label: "Start here",
          items: [
            { label: "What Stannum is", slug: "start/introduction" },
            { label: "Download", slug: "start/download" },
            { label: "Build from source", slug: "start/install" },
            { label: "Open a case", slug: "start/open-a-case" },
          ],
        },
        {
          label: "User guide",
          items: [
            { label: "The window", slug: "guide/shell" },
            { label: "Find anything (Ctrl+K)", slug: "guide/command-palette" },
            { label: "Log", slug: "guide/log" },
            { label: "Side by side", slug: "guide/side-by-side" },
            { label: "Studies", slug: "guide/studies" },
            { label: "Diagram", slug: "guide/diagram" },
            { label: "Power flow", slug: "guide/power-flow" },
            { label: "Contingencies", slug: "guide/contingencies" },
            { label: "Records", slug: "guide/records" },
            { label: "Topology", slug: "guide/topology" },
            { label: "Case diff", slug: "guide/case-diff" },
            { label: "Run files", slug: "guide/run-files" },
            { label: "Python", slug: "guide/python" },
            { label: "Keyboard shortcuts", slug: "guide/shortcuts" },
          ],
        },
        {
          label: "Technical manual",
          items: [
            { label: "Architecture", slug: "manual/architecture" },
            { label: "Network model", slug: "manual/network-model" },
            { label: "AC Newton power flow", slug: "manual/ac-power-flow" },
            { label: "AC solution methods", slug: "manual/ac-methods" },
            { label: "DC (linear) power flow", slug: "manual/dc-power-flow" },
            { label: "Generator reactive limits", slug: "manual/reactive-limits" },
            { label: "Remote voltage regulation", slug: "manual/remote-regulation" },
            { label: "Switched shunts and taps", slug: "manual/voltage-controls" },
            { label: "Phase shifters", slug: "manual/phase-shifters" },
            { label: "Sensitivities, PTDF and LODF", slug: "manual/sensitivities" },
            { label: "Contingency analysis", slug: "manual/contingencies" },
            {
              label: "HVDC",
              items: [
                { label: "Overview and simplified model", slug: "manual/hvdc/overview" },
                { label: "Two-terminal LCC", slug: "manual/hvdc/lcc" },
                { label: "Two-terminal VSC", slug: "manual/hvdc/vsc" },
                { label: "Multi-terminal DC", slug: "manual/hvdc/mtdc" },
                { label: "Meshed VSC grids", slug: "manual/hvdc/vsc-grids" },
              ],
            },
            { label: "Topology analysis", slug: "manual/topology" },
            { label: "Case diff", slug: "manual/case-diff" },
            { label: "Synthetic test cases", slug: "manual/synthetic-cases" },
          ],
        },
        {
          label: "Developer guide",
          items: [
            { label: "Codebase tour", slug: "develop/overview" },
            { label: "Set up and run", slug: "develop/setup" },
            { label: "Crates", slug: "develop/crates" },
            { label: "The command registry", slug: "develop/commands" },
            { label: "Add a command", slug: "develop/add-a-command" },
            { label: "Desktop shell", slug: "develop/desktop-shell" },
            { label: "Frontend", slug: "develop/frontend" },
            { label: "Tests and CI", slug: "develop/testing" },
          ],
        },
        {
          label: "File formats",
          items: [
            { label: "PSS/E RAW", slug: "reference/psse-raw" },
            { label: "MATPOWER", slug: "reference/matpower" },
            { label: "Stannum DC grid file", slug: "reference/dc-grid-file" },
            { label: "Bus coordinates file", slug: "reference/coordinates-file" },
            { label: "Run file format", slug: "reference/run-file" },
          ],
        },
        {
          label: "Reference",
          items: [
            { label: "Verification", slug: "reference/verification" },
            { label: "What isn't modelled", slug: "reference/limitations" },
            { label: "Glossary", slug: "reference/glossary" },
          ],
        },
      ],
    }),
  ],
});
