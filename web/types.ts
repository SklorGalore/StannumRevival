export interface Diagnostic { severity:'error'|'warning'|'info'; section:string; line:number; message:string }
export interface SourceRecord { line:number; section:string; fields:Record<string,string>; raw_lines:string[] }
export interface Bus { number:number; name:string; base_kv:number; bus_type:number; area:number|null; zone:number|null; voltage_pu:number|null; source:SourceRecord }
export interface Equipment { id:string; kind:'line'|'transformer'|'switch'; name:string; circuit:string; buses:number[]; status:number; active_terminals:boolean[]; source:SourceRecord }
export interface Injection { id:string; bus:number; status:number; p_mw:number; q_mvar:number; source:SourceRecord }
export interface PowerCase { revision:number; base_mva:number; frequency_hz:number; titles:string[]; encoding:string; buses:Bus[]; equipment:Equipment[]; loads:Injection[]; generators:Injection[]; diagnostics:Diagnostic[]; skipped_sections:Record<string,number> }
export interface Connection { id:string; equipment_id:string; source:string; target:string; winding:number|null }
export interface GraphNode { id:string; label:string; bus:number|null; equipment_id:string|null; base_kv:number|null }
export interface GraphData { nodes:GraphNode[]; edges:Connection[] }
export interface Island { id:number; buses:number[]; synthetic_nodes:string[] }
export interface Degree { bus:number; degree:number; island:number }
export interface Analysis { active_buses:number; excluded_buses:number; active_equipment:number; excluded_equipment:number; synthetic_junctions:number; islands:Island[]; degrees:Degree[]; bridges:Connection[]; articulation_buses:number[]; synthetic_articulations:string[] }
export interface Path { buses:number[]; equipment_ids:string[]; node_ids:string[]; connection_ids:string[]; hops:number }
export interface Snapshot { case:PowerCase; analysis:Analysis; graph:GraphData }
export type WorkerRequest = { id:number; type:'load'; bytes:ArrayBuffer } | { id:number; type:'path'; from:number; to:number };
export type WorkerResponse = { id:number; type:'loaded'; snapshot:Snapshot } | { id:number; type:'path'; path:Path|null } | { id:number; type:'error'; error:Diagnostic };
