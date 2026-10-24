# Nodal design philosophy

## Direction

Nodal is a technical workbench for experienced power-system engineers. Its primary theme is light: a calm, precise workspace for sustained investigation of electrical network topology.

The audience, workbench direction, light theme, and design philosophy were approved on 2026-10-10. The graph-first layout was subsequently selected from the visual comparison. This document is a design reference; it does not change the application.

Every visual choice should help an engineer locate, compare, trace, or verify something.

## Principles

### Evidence before presentation

Keep bus numbers, circuit identifiers, units, equipment status, and source records easy to reach. A finding should lead directly to its supporting equipment and original RAW fields. Preserve the distinction between source data and derived analysis.

### Density with hierarchy

Use compact tables and controls, clear alignment, and deliberate grouping. Give graphs and results useful space. Avoid oversized decorative headings, excessive cards, and tiny or low-contrast labels. Density must not require smaller text to remain usable.

### Stable context

Keep the current case, selected element, graph scope, and relevant filters apparent. Moving between results, graph selections, and inspection should preserve orientation. Distinguish selection from hover and keyboard focus.

### Color carries meaning

Use neutral colors for the ordinary network. Reserve emphasis for selection, paths, findings, warnings, and errors. Reinforce meaning with text, symbols, or line styles so color is never the only signal.

### Precise claims

Use established engineering terminology. Explain application-specific interpretations, including equipment-hop counting and synthetic transformer junctions. A topology bridge is a connectivity finding, not an outage consequence. Make coverage limits and excluded equipment visible alongside the affected results.

### Fast, repeatable interaction

Favor predictable selection, searchable identifiers, keyboard access, and short routes from a result to its evidence. Controls should have explicit labels and visible feedback. Opening a local case should feel as direct as opening a document.

## Light workbench theme

### Surfaces and structure

Use a cool gray workspace, white analysis surfaces, and deep navy navigation. Thin borders establish boundaries. Keep shadows minimal and corner rounding restrained. Use whitespace to separate tasks, without spreading related information across disconnected cards.

The graph, results, and inspector are the central work surfaces. Supporting case metadata and summary counts should be compact enough to leave those surfaces room.

### Typography

Use a readable sans serif for navigation, controls, and descriptive text. Use tabular numerals for measurements and numeric table columns. Use monospace selectively for identifiers and original RAW records.

Use a small, consistent type hierarchy. Normal labels and table data must remain legible at the default browser scale; tiny uppercase text must not carry essential information. Show units beside values or in explicit column headings.

### Color semantics

| Meaning | Proposed treatment |
| --- | --- |
| Workspace | Cool light gray |
| Analysis surfaces | White |
| Navigation and primary text | Deep navy and dark neutral tones |
| Secondary text | Neutral tone with readable contrast |
| Primary actions and selection | Teal |
| Traced path | Blue, with a distinct line treatment |
| Topology findings | Amber, with a finding label or symbol |
| Coverage warnings | Amber, with explicit warning text |
| Errors | Red, with an actionable diagnostic |
| Inactive or excluded equipment | Muted neutral, with explicit status |

Exact color values are an implementation decision. Validate text contrast, focus visibility, and distinctions between overlapping graph states before adopting them. Do not use green or teal alone to imply that a network is safe or operationally healthy.

### Graph presentation

Keep ordinary nodes and connections visually quiet. Selection and tracing should establish the hierarchy. Make labels useful at the current zoom rather than attempting to show every label at once. Legends should describe the states actually displayed.

Clearly distinguish physical buses, synthetic junctions, selected elements, traced paths, and topology findings. When states overlap, retain evidence of each relevant state through labels, outlines, or line styles.

### Controls and icons

Use consistent control heights, spacing, and interaction states. Primary actions should stand out without dominating the analysis. Replace inconsistent decorative glyphs with a coherent technical icon set when updating the interface; retain text labels for important actions.

### Language

Use direct terms such as “Open case,” “Bus neighborhood,” and “Equipment hops.” Assume knowledge of electrical engineering, while explaining Nodal's analysis rules and limitations. Empty states should name the next useful action. Diagnostics should identify the issue, its source location when available, and its effect on analysis.

## Review criteria

Evaluate future interface changes against these questions:

- Can an engineer locate and compare identifiers and measurements quickly?
- Can each finding be traced to source data?
- Are selection, scope, units, status, and coverage clear?
- Does the layout leave useful room for graphs, tables, and inspection?
- Are text, controls, and focus states readable without relying on color alone?
- Does the change reduce the effort required for repeated investigation?

## Scope

This document establishes the overall design philosophy and selected layout direction. It does not define a component specification or implementation plan. Dark mode may be considered later; the primary design is light.

## Selected layout: graph first

The focused network graph leads the analysis area. The searchable results table sits directly beneath it, and a persistent inspector occupies the right column. Selecting a graph element or result row updates the same inspector and preserves the surrounding investigation context.

Keep the case header and summary counts compact. Place path controls below the analysis area, with explicit endpoint identifiers and an equipment-hop result. Keep export and case-opening actions accessible in the case header.

On narrower screens, stack the graph, inspector, and results in a readable order. The desktop priority must not force cramped columns or unreadable data on smaller screens.

The selected visual concept is saved in `.superpowers/brainstorm/20512-1791661353/content/workbench-layout.html`. Its data is illustrative. Selecting the layout does not validate its sample network or establish final component dimensions.

## Proposed implementation values

These values accompanied the selected concept. They are starting values for implementation and should be checked for contrast and graph-state clarity in the actual interface.

| Role | Value |
| --- | --- |
| Workspace | `#F3F5F7` |
| Analysis surface | `#FFFFFF` |
| Navigation | `#142B3C` |
| Selection and primary action | `#0F766E` |
| Traced path | `#2563EB` |
| Finding text | `#986315` |

Use a system sans serif with a proposed scale of 14 px for data and controls, 12 px for secondary labels, 18 px for section titles, and 24 px for the case title. Use tabular numerals for measurements and selective monospace for identifiers and source records. The comparison mockups were scaled illustrations rather than final typography specifications.
