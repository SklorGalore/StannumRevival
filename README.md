# StannumSLD

A desktop application for creating, editing, and exporting power system single-line diagrams (SLDs). Built with Tauri 2, React, and TypeScript.

![StannumSLD layout: component palette on the left, diagram canvas in the center, properties panel on the right]

## Features

- **Drag-and-drop diagram editor** — place and connect power system components on an infinite canvas with pan, zoom, and snap-to-grid
- **IEEE 315 / ANSI Y32.2 symbols** — industry-standard SVG symbols for all component types
- **Interactive components** — click circuit breakers and disconnectors to toggle their state directly on the canvas
- **Properties panel** — select any component or conductor to edit its electrical parameters
- **Export** — save diagrams as SVG, PNG (96/150/300 DPI), or PDF with an engineering title block
- **Save / Load** — persist diagrams to `.sld` JSON files and reopen them later

## Component Library

| Category | Components |
|---|---|
| Generation | Generator (synchronous, with grounding options) |
| Network | Bus (voltage-level color-coded) |
| Transformation | Two-winding Transformer (Wye/Delta connection symbols) |
| Protection | Circuit Breaker, Disconnector / Isolator, Fuse |
| Load | Static Load, Motor Load |

### Bus voltage color coding

| Voltage | Color |
|---|---|
| ≥ 765 kV | Red |
| 345–500 kV | Orange |
| 115–230 kV | Yellow |
| 34.5–69 kV | Green |
| < 34.5 kV | Blue |

## Connection Rules

- **Bus → Bus** connections are forbidden (route through a breaker or switch)
- **Load** nodes are terminals — they cannot be the source of a connection
- All conductors carry configurable voltage, type (transmission / distribution / buswork), and impedance values

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Tauri CLI](https://tauri.app/start/prerequisites/) v2

### Install dependencies

```bash
npm install
```

### Run in development

```bash
npm run tauri dev
```

### Build for production

```bash
npm run tauri build
```

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Delete` / `Backspace` | Delete selected components |
| `Ctrl+S` | Save diagram |
| `Shift+click` | Multi-select |
| `Scroll` | Zoom in / out |
| `Middle-click drag` | Pan canvas |

## File Format

Diagrams are saved as `.sld` files (JSON). The format is stable and human-readable:

```json
{
  "metadata": {
    "title": "Example Substation",
    "revision": "A",
    "company": "Acme Power",
    "schemaVersion": "1.0"
  },
  "nodes": [...],
  "edges": [...],
  "viewport": { "x": 0, "y": 0, "zoom": 1 }
}
```

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop shell | [Tauri 2](https://tauri.app/) (Rust) |
| Frontend | [React 18](https://react.dev/) + [TypeScript](https://www.typescriptlang.org/) |
| Diagram canvas | [React Flow 12](https://reactflow.dev/) (`@xyflow/react`) |
| State management | [Zustand 5](https://zustand-demo.pmnd.rs/) + Immer |
| Styling | [Tailwind CSS 4](https://tailwindcss.com/) |
| PDF export | [jsPDF](https://github.com/parallax/jsPDF) |
| Build tool | [Vite](https://vitejs.dev/) |

## Project Structure

```
src/
├── components/
│   ├── canvas/       # React Flow host, drop target
│   ├── dialogs/      # Export and metadata dialogs
│   ├── edges/        # Conductor edge renderer
│   ├── layout/       # AppShell, Toolbar, StatusBar
│   ├── nodes/        # One file per IEEE symbol component
│   ├── palette/      # Draggable component palette
│   └── properties/   # Per-type property forms
├── hooks/            # Keyboard shortcuts
├── lib/
│   ├── export/       # SVG → PNG → PDF pipeline
│   ├── serialization/
│   └── tauri/        # Native file dialog wrappers
└── store/            # Zustand store, types, defaults
src-tauri/
└── src/lib.rs        # save_diagram, load_diagram, save_binary commands
```
