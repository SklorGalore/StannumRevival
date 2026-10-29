{
  "metadata": {
    "title": "five_bus",
    "revision": "A",
    "company": "",
    "description": "One-line generated from five_bus.raw",
    "createdAt": "2026-10-28T20:42:08.116Z",
    "modifiedAt": "2026-10-28T20:42:12.375Z",
    "schemaVersion": "1.0",
    "caseName": "five_bus.raw"
  },
  "nodes": [
    {
      "id": "g0",
      "type": "generator",
      "position": {
        "x": 340,
        "y": 26
      },
      "data": {
        "label": "G 1",
        "nominalMW": 300,
        "voltageKV": 138,
        "powerFactor": 0.85,
        "groundingType": "solidly",
        "caseRef": {
          "kind": "gen",
          "id": 0,
          "element": "generator:1:1"
        },
        "labelOffset": {
          "x": 65,
          "y": 45
        }
      },
      "measured": {
        "width": 40,
        "height": 80
      },
      "selected": false
    },
    {
      "id": "g1",
      "type": "generator",
      "position": {
        "x": 520,
        "y": 506
      },
      "data": {
        "label": "G 3",
        "nominalMW": 300,
        "voltageKV": 138,
        "powerFactor": 0.85,
        "groundingType": "solidly",
        "caseRef": {
          "kind": "gen",
          "id": 1,
          "element": "generator:3:1"
        }
      },
      "measured": {
        "width": 40,
        "height": 80
      },
      "selected": false
    },
    {
      "id": "l2",
      "type": "load",
      "position": {
        "x": 160,
        "y": 226
      },
      "data": {
        "label": "Load",
        "nominalMW": 90,
        "nominalMVAR": 30,
        "loadType": "static",
        "caseRef": {
          "kind": "load",
          "id": 2,
          "element": "bus:2"
        }
      },
      "measured": {
        "width": 40,
        "height": 60
      },
      "selected": false
    },
    {
      "id": "l4",
      "type": "load",
      "position": {
        "x": 100,
        "y": 526
      },
      "data": {
        "label": "Load",
        "nominalMW": 40,
        "nominalMVAR": 12,
        "loadType": "static",
        "caseRef": {
          "kind": "load",
          "id": 4,
          "element": "bus:4"
        }
      },
      "measured": {
        "width": 40,
        "height": 60
      },
      "selected": false
    },
    {
      "id": "l5",
      "type": "load",
      "position": {
        "x": 140,
        "y": 726
      },
      "data": {
        "label": "Load",
        "nominalMW": 20,
        "nominalMVAR": 6,
        "loadType": "static",
        "caseRef": {
          "kind": "load",
          "id": 5,
          "element": "bus:5"
        }
      },
      "measured": {
        "width": 40,
        "height": 60
      },
      "selected": false
    },
    {
      "id": "b1",
      "type": "bus",
      "position": {
        "x": 340,
        "y": 20
      },
      "data": {
        "label": "North_138",
        "voltageKV": 138,
        "orientation": "horizontal",
        "length": 220,
        "taps": [
          {
            "id": "t0",
            "x": 160,
            "side": "bottom"
          },
          {
            "id": "t1",
            "x": 200,
            "side": "bottom"
          },
          {
            "id": "i0",
            "x": 20,
            "side": "bottom"
          }
        ],
        "caseRef": {
          "kind": "bus",
          "id": 1,
          "element": "bus:1"
        }
      },
      "measured": {
        "width": 220,
        "height": 6
      },
      "selected": false
    },
    {
      "id": "b2",
      "type": "bus",
      "position": {
        "x": 160,
        "y": 220
      },
      "data": {
        "label": "Central_138",
        "voltageKV": 138,
        "orientation": "horizontal",
        "length": 260,
        "taps": [
          {
            "id": "t0",
            "x": 240,
            "side": "top"
          },
          {
            "id": "t1",
            "x": 240,
            "side": "bottom"
          },
          {
            "id": "t2",
            "x": 200,
            "side": "bottom"
          },
          {
            "id": "i0",
            "x": 20,
            "side": "bottom"
          }
        ],
        "caseRef": {
          "kind": "bus",
          "id": 2,
          "element": "bus:2"
        }
      },
      "measured": {
        "width": 260,
        "height": 6
      },
      "selected": false
    },
    {
      "id": "b3",
      "type": "bus",
      "position": {
        "x": 520,
        "y": 500
      },
      "data": {
        "label": "East_138",
        "voltageKV": 138,
        "orientation": "horizontal",
        "length": 220,
        "taps": [
          {
            "id": "t0",
            "x": 160,
            "side": "top"
          },
          {
            "id": "t1",
            "x": 200,
            "side": "top"
          },
          {
            "id": "i0",
            "x": 20,
            "side": "bottom"
          }
        ],
        "caseRef": {
          "kind": "bus",
          "id": 3,
          "element": "bus:3"
        }
      },
      "measured": {
        "width": 220,
        "height": 6
      },
      "selected": false
    },
    {
      "id": "b4",
      "type": "bus",
      "position": {
        "x": 100,
        "y": 520
      },
      "data": {
        "label": "Central_69",
        "voltageKV": 69,
        "orientation": "horizontal",
        "length": 220,
        "taps": [
          {
            "id": "t0",
            "x": 200,
            "side": "bottom"
          },
          {
            "id": "t1",
            "x": 200,
            "side": "top"
          },
          {
            "id": "i0",
            "x": 20,
            "side": "bottom"
          }
        ],
        "caseRef": {
          "kind": "bus",
          "id": 4,
          "element": "bus:4"
        }
      },
      "measured": {
        "width": 220,
        "height": 6
      },
      "selected": false
    },
    {
      "id": "b5",
      "type": "bus",
      "position": {
        "x": 140,
        "y": 720
      },
      "data": {
        "label": "Spur_69",
        "voltageKV": 69,
        "orientation": "horizontal",
        "length": 180,
        "taps": [
          {
            "id": "t0",
            "x": 160,
            "side": "top"
          },
          {
            "id": "i0",
            "x": 20,
            "side": "bottom"
          }
        ],
        "caseRef": {
          "kind": "bus",
          "id": 5,
          "element": "bus:5"
        }
      },
      "measured": {
        "width": 180,
        "height": 6
      },
      "selected": false
    },
    {
      "id": "t4",
      "type": "transformer",
      "position": {
        "x": 280,
        "y": 340
      },
      "data": {
        "label": "T 2-4",
        "ratingMVA": 146.6,
        "primaryKV": 138,
        "secondaryKV": 69,
        "connection": "WyeDelta",
        "groundingPrimary": "solidly",
        "groundingSecondary": "none",
        "impedancePct": 8,
        "windingsUnknown": true,
        "caseRef": {
          "kind": "branch",
          "id": 4,
          "element": "transformer:2:4:1"
        }
      },
      "measured": {
        "width": 40,
        "height": 100
      },
      "selected": false
    }
  ],
  "edges": [
    {
      "id": "e-br0",
      "type": "conductor",
      "source": "b1",
      "target": "b2",
      "sourceHandle": "t0",
      "targetHandle": "t0",
      "data": {
        "voltageKV": 138,
        "conductorType": "transmission",
        "caseRef": {
          "kind": "branch",
          "id": 0,
          "element": "line:1:2:1"
        },
        "route": [
          {
            "x": 500,
            "y": 26
          },
          {
            "x": 500,
            "y": 120
          },
          {
            "x": 400,
            "y": 120
          },
          {
            "x": 400,
            "y": 220
          }
        ],
        "labelAt": {
          "x": 327,
          "y": 146,
          "width": 64,
          "height": 34
        },
        "labelOffset": {
          "x": -65,
          "y": 0
        }
      },
      "selected": true
    },
    {
      "id": "e-br1",
      "type": "conductor",
      "source": "b2",
      "target": "b3",
      "sourceHandle": "t1",
      "targetHandle": "t0",
      "data": {
        "voltageKV": 138,
        "conductorType": "transmission",
        "caseRef": {
          "kind": "branch",
          "id": 1,
          "element": "line:2:3:1"
        },
        "route": [
          {
            "x": 400,
            "y": 226
          },
          {
            "x": 400,
            "y": 320
          },
          {
            "x": 520,
            "y": 320
          },
          {
            "x": 520,
            "y": 480
          },
          {
            "x": 680,
            "y": 480
          },
          {
            "x": 680,
            "y": 500
          }
        ],
        "labelAt": {
          "x": 456,
          "y": 375,
          "width": 64,
          "height": 34
        }
      },
      "selected": false
    },
    {
      "id": "e-br2",
      "type": "conductor",
      "source": "b1",
      "target": "b3",
      "sourceHandle": "t1",
      "targetHandle": "t1",
      "data": {
        "voltageKV": 138,
        "conductorType": "transmission",
        "caseRef": {
          "kind": "branch",
          "id": 2,
          "element": "line:1:3:1"
        },
        "route": [
          {
            "x": 540,
            "y": 26
          },
          {
            "x": 540,
            "y": 460
          },
          {
            "x": 720,
            "y": 460
          },
          {
            "x": 720,
            "y": 500
          }
        ],
        "labelAt": {
          "x": 550,
          "y": 244,
          "width": 64,
          "height": 34
        }
      },
      "selected": false
    },
    {
      "id": "e-br3",
      "type": "conductor",
      "source": "b4",
      "target": "b5",
      "sourceHandle": "t0",
      "targetHandle": "t0",
      "data": {
        "voltageKV": 69,
        "conductorType": "distribution",
        "caseRef": {
          "kind": "branch",
          "id": 3,
          "element": "line:4:5:1"
        },
        "route": [
          {
            "x": 300,
            "y": 526
          },
          {
            "x": 300,
            "y": 720
          }
        ],
        "labelAt": {
          "x": 228,
          "y": 636,
          "width": 64,
          "height": 34
        }
      },
      "selected": false
    },
    {
      "id": "e-t4-hv",
      "type": "conductor",
      "source": "b2",
      "target": "t4",
      "sourceHandle": "t2",
      "targetHandle": "xfmr-primary",
      "data": {
        "voltageKV": 138,
        "conductorType": "transmission",
        "route": [
          {
            "x": 360,
            "y": 226
          },
          {
            "x": 360,
            "y": 320
          },
          {
            "x": 300,
            "y": 320
          },
          {
            "x": 300,
            "y": 340
          }
        ]
      },
      "selected": false
    },
    {
      "id": "e-t4-lv",
      "type": "conductor",
      "source": "t4",
      "target": "b4",
      "sourceHandle": "xfmr-secondary",
      "targetHandle": "t1",
      "data": {
        "voltageKV": 69,
        "conductorType": "distribution",
        "route": [
          {
            "x": 300,
            "y": 440
          },
          {
            "x": 300,
            "y": 520
          }
        ]
      },
      "selected": false
    },
    {
      "id": "e-g0",
      "type": "conductor",
      "source": "b1",
      "sourceHandle": "i0",
      "target": "g0",
      "targetHandle": "gen-top",
      "data": {
        "voltageKV": 138,
        "conductorType": "transmission"
      },
      "selected": false
    },
    {
      "id": "e-g1",
      "type": "conductor",
      "source": "b3",
      "sourceHandle": "i0",
      "target": "g1",
      "targetHandle": "gen-top",
      "data": {
        "voltageKV": 138,
        "conductorType": "transmission"
      },
      "selected": false
    },
    {
      "id": "e-l2",
      "type": "conductor",
      "source": "b2",
      "sourceHandle": "i0",
      "target": "l2",
      "targetHandle": "load-top",
      "data": {
        "voltageKV": 138,
        "conductorType": "transmission"
      },
      "selected": false
    },
    {
      "id": "e-l4",
      "type": "conductor",
      "source": "b4",
      "sourceHandle": "i0",
      "target": "l4",
      "targetHandle": "load-top",
      "data": {
        "voltageKV": 69,
        "conductorType": "distribution"
      },
      "selected": false
    },
    {
      "id": "e-l5",
      "type": "conductor",
      "source": "b5",
      "sourceHandle": "i0",
      "target": "l5",
      "targetHandle": "load-top",
      "data": {
        "voltageKV": 69,
        "conductorType": "distribution"
      },
      "selected": false
    }
  ],
  "viewport": {
    "x": 139.50913838120107,
    "y": 16.167101827676277,
    "zoom": 0.891644908616188
  }
}