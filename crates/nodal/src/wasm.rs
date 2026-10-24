use crate::{model::Case, topology::Topology};
use serde::Serialize;
use wasm_bindgen::prelude::*;

fn js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// The worker owns this handle until another file is opened or the tab closes.
#[wasm_bindgen]
pub struct NodalCase {
    case: Case,
    topology: Topology,
}

#[wasm_bindgen(js_name = parse_raw)]
pub fn parse_raw_wasm(bytes: &[u8]) -> Result<NodalCase, JsValue> {
    let case = crate::parse_raw(bytes)
        .map_err(|e| js(&e).unwrap_or_else(|_| JsValue::from_str(&e.to_string())))?;
    let topology = Topology::new(&case);
    Ok(NodalCase { case, topology })
}

#[wasm_bindgen]
impl NodalCase {
    pub fn snapshot(&self) -> Result<JsValue, JsValue> {
        #[derive(Serialize)]
        struct Snapshot<'a> {
            case: &'a Case,
            analysis: crate::topology::Analysis,
            graph: crate::topology::GraphData,
        }
        js(&Snapshot {
            case: &self.case,
            analysis: self.topology.analyze(&self.case),
            graph: self.topology.graph_data(),
        })
    }

    pub fn analyze(&self) -> Result<JsValue, JsValue> {
        js(&self.topology.analyze(&self.case))
    }

    pub fn set_merge_zero_impedance(&mut self, enabled: bool) {
        self.topology = Topology::with_options(
            &self.case,
            crate::topology::TopologyOptions {
                merge_zero_impedance: enabled,
            },
        );
    }

    pub fn shortest_path(&self, from_bus: i32, to_bus: i32) -> Result<JsValue, JsValue> {
        let path = self
            .topology
            .shortest_path(&self.case, from_bus, to_bus)
            .map_err(|e| js(&e).unwrap_or_else(|_| JsValue::from_str(&e.to_string())))?;
        js(&path)
    }
}
