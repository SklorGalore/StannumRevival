import {describe,it,expect} from 'vitest';
import {busRows,memberSearchText} from './topology-view';
import {nodeForBus,neighborhood} from './view-model';
import type {Snapshot} from './types';

const src={line:1,section:'BUS',fields:{},raw_lines:[]};
const base:Snapshot={case:{revision:35,base_mva:100,frequency_hz:60,zero_impedance_threshold:.001,titles:[],encoding:'UTF-8',buses:[1,2,3,4].map((number)=>({number,name:`Bus ${number}`,base_kv:138,bus_type:number===4?4:1,area:1,zone:1,voltage_pu:1,source:src})),equipment:[],loads:[],generators:[],diagnostics:[],skipped_sections:{}},analysis:{active_buses:3,original_active_buses:3,absorbed_equipment:0,excluded_buses:1,active_equipment:1,excluded_equipment:0,synthetic_junctions:0,islands:[{id:1,buses:[1,3],synthetic_nodes:[]}],degrees:[{bus:1,degree:1,island:1},{bus:3,degree:1,island:1}],bridges:[],articulation_buses:[1],synthetic_articulations:[]},graph:{nodes:[{id:'bus:1',label:'1',bus:1,equipment_id:null,base_kv:138,member_buses:[1,2]},{id:'bus:3',label:'3',bus:3,equipment_id:null,base_kv:138,member_buses:[3]}],edges:[{id:'e',equipment_id:'line:1:3:1',source:'bus:1',target:'bus:3',winding:null}],zero_impedance_groups:[[1,2]],merge_zero_impedance:true,absorbed_equipment_ids:[]}};

describe('merged topology views',()=>{
  it('resolves original member buses to their canonical node and neighborhood',()=>{
    expect(nodeForBus(base.graph,2)?.bus).toBe(1);
    expect(neighborhood(base.graph,2,0).nodes.map(n=>n.id)).toEqual(['bus:1']);
  });
  it('emits one row per canonical node and keeps excluded source buses',()=>{
    const rows=busRows(base);
    expect(rows.map(row=>row.bus)).toEqual([1,3,4]);
    expect(rows[0].member_buses).toEqual([1,2]);
    expect(rows[0]).toMatchObject({degree:1,island:1,articulation:true});
    expect(rows.at(-1)).toMatchObject({bus:4,status:'Excluded',member_buses:[4]});
  });
  it('searches member numbers and names without adding table rows',()=>{
    const node=base.graph.nodes[0];
    expect(memberSearchText(base.graph,base.case.buses,node)).toContain('2 bus 2');
    expect(busRows(base)).toHaveLength(3);
  });
});
