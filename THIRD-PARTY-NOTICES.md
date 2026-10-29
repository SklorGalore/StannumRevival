# Third-party notices

Stannum is BSD-3-Clause (see `LICENSE`). It includes, adapts or draws on the
work below, which keeps its own terms. The licence of every Rust crate and
npm package built into Stannum, with its full text, is in
`THIRD-PARTY-LICENSES.txt`. Both files ship with the installers, the Python
wheels and the `stannum-cli` archives.

## MATPOWER (BSD-3-Clause)

The power flow in Stannum (the `rs-power` crate) adapts formulations and
result handling from MATPOWER 8.1 (https://matpower.org,
https://github.com/MATPOWER/matpower/tree/8.1): makeYbus, makeBdc, makeSbus,
newtonpf, dSbus_dV, makeB, pfsoln, bustypes, ext2int, int2ext, runpf, and the
PSS/E RAW conversion routines (psse_read, psse_parse, psse_parse_line,
psse_convert, psse_convert_xfmr, psse_convert_hvdc). The RAW conversions
derive from work by Yujia Zhu and Ray Zimmerman, Copyright (c) 2014-2024,
Power Systems Engineering Research Center (PSERC). Its licence must
accompany redistributions:

```text
Copyright (c) 1996-2025, Power Systems Engineering Research Center (PSERC)
and individual contributors (see AUTHORS file for details).
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

1. Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS
IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

--------------------------------------------------------------------------
```

MATPOWER's case files are excluded from its software licence, and Stannum's
licence grants nothing for them either. One is built into Stannum as a
sample case: `case9.m` (MATPOWER 8.1, `data/case9.m`), unchanged, with its
original header naming its source (J. H. Chow, ed., *Time-Scale Modeling of
Dynamic Networks with Applications to Power Systems*, Springer-Verlag, 1982,
p. 70, from R. P. Schulz, A. E. Turner and D. N. Ewart, "Long Term Power
System Dynamics," EPRI Report 90-7-0, 1974). The others in the source
repository are test fixtures and keep their own provenance (see
`crates/rs-power/NOTICE`). Stannum's other sample cases are its own made-up
data (see `crates/stannum-core/samples/README.md`).

## Eclipse Layout Kernel, via elkjs (EPL-2.0)

The diagram's automatic layouts use elkjs (https://github.com/kieler/elkjs),
a JavaScript build of the Eclipse Layout Kernel (https://github.com/eclipse/elk),
Copyright (c) the Kiel University and the Eclipse Layout Kernel contributors.
elkjs is offered under EPL-2.0 or GPL-3.0-or-later; Stannum uses it under the
Eclipse Public License 2.0 (https://www.eclipse.org/legal/epl-2.0/), unmodified.
Its source code is available from the links above, and its full licence text
is in `THIRD-PARTY-LICENSES.txt`. The EPL-2.0 covers elkjs only, not the rest
of Stannum.

## MapLibre GL JS (BSD-3-Clause)

The map view is drawn with MapLibre GL JS (https://github.com/maplibre/maplibre-gl-js),
Copyright (c) 2023, MapLibre contributors, with parts Copyright (c) 2020,
Mapbox. Its licence, and those of the small libraries it uses, are in
`THIRD-PARTY-LICENSES.txt`.

## Natural Earth (public domain)

The map's offline coastlines, lakes, and country and state borders
(`public/basemap/outlines.json`) are simplified from Natural Earth 1:50m data
(https://www.naturalearthdata.com), which is in the public domain. Made with
Natural Earth. Free vector and raster map data @ naturalearthdata.com.

## Packages under MPL-2.0

These Rust crates are used unmodified under the Mozilla Public License 2.0
(https://mozilla.org/MPL/2.0/). Their source code is available from crates.io
and from the repositories listed in `THIRD-PARTY-LICENSES.txt`:

- cssparser, cssparser-macros, selectors (Servo project), dtoa-short and
  option-ext, which come with the Tauri desktop shell;
- priority-queue, which nodal's topology analysis uses, offered under
  LGPL-3.0-or-later or MPL-2.0 and used under MPL-2.0.

The DOMPurify npm package, which jsPDF loads, is offered under MPL-2.0 or
Apache-2.0 and used under Apache-2.0.

## Methods from published papers

Some features follow methods from published papers rather than code: the
synthetic cases (Birchfield et al., 2017) and the VSC multi-terminal DC power
flow (Beerten, Cole and Belmans, 2012). These were written from the papers,
and no code from their authors' tools is included.
