// Ported from nodal's web/field-colors.test.ts.
import {describe,expect,it} from 'vitest';
import {createColorPresentation,defaultColorSettings,loadColorSettings,saveColorSettings} from './field-colors';
import type {Snapshot} from './types';

const source=(line:number)=>({line,section:'BUS',fields:{},raw_lines:[]});

function makeSnapshot():Snapshot {
  const buses=[
    {number:1,name:'Alpha',base_kv:138,bus_type:1,area:1,zone:10,voltage_pu:1.01,source:source(1)},
    {number:2,name:'Beta',base_kv:138,bus_type:2,area:2,zone:10,voltage_pu:null,source:source(2)},
    {number:3,name:'Gamma',base_kv:69,bus_type:4,area:null,zone:20,voltage_pu:0,source:source(3)},
  ];
  const equipment=[
    {id:'line:1:2:1',kind:'line' as const,name:'Alpha-Beta',circuit:'1',buses:[1,2],status:1,active_terminals:[true,true],source:source(4)},
    {id:'switch:2:3:1',kind:'switch' as const,name:'Tie',circuit:'1',buses:[2,3],status:0,active_terminals:[false,false],source:source(5)},
  ];
  const graph={
    nodes:[
      {id:'bus:1',label:'1 · 2',bus:1,equipment_id:null,base_kv:138,member_buses:[1,2]},
      {id:'bus:3',label:'3',bus:3,equipment_id:null,base_kv:69,member_buses:[3]},
      {id:'junction:switch:2:3:1',label:'T',bus:null,equipment_id:'switch:2:3:1',base_kv:null,member_buses:[]},
    ],
    edges:[
      {id:'e1',equipment_id:'line:1:2:1',source:'bus:1',target:'bus:1',winding:null},
      {id:'e2',equipment_id:'switch:2:3:1',source:'bus:3',target:'junction:switch:2:3:1',winding:1},
    ],zero_impedance_groups:[[1,2]],merge_zero_impedance:true,absorbed_equipment_ids:[],
  };
  return {
    case:{revision:35,base_mva:100,frequency_hz:60,zero_impedance_threshold:0,titles:[],encoding:'UTF-8',buses,equipment,loads:[],generators:[],diagnostics:[],skipped_sections:{}},
    graph,
    analysis:{active_buses:2,original_active_buses:3,absorbed_equipment:0,excluded_buses:0,active_equipment:2,excluded_equipment:0,synthetic_junctions:1,islands:[],degrees:[{bus:1,degree:2,island:1},{bus:3,degree:1,island:1}],bridges:[],articulation_buses:[],synthetic_articulations:[]},
  };
}

describe('field color presentation',()=>{
  it('colors buses by stable category domains and represents merged conflicts as mixed',()=>{
    const settings=defaultColorSettings();
    settings.bus.field='area';
    settings.bus.colors['1']='#112233';
    settings.bus.mixed='#aabbcc';
    const result=createColorPresentation(makeSnapshot(),settings);
    expect(result.nodeColors['bus:1']).toBe('#aabbcc');
    expect(result.nodeColors['bus:3']).toMatch(/^#[0-9a-f]{6}$/i);
    expect(result.nodeColors).not.toHaveProperty('junction:switch:2:3:1');
    expect(result.legends[0].entries.map(entry=>entry.label)).toEqual(['1','2','Missing']);
    expect(result.legends[0].entries.find(entry=>entry.label==='1')?.color).toBe('#112233');
  });

  it('maps numeric values against the full case domain and clamps to configured bounds',()=>{
    const settings=defaultColorSettings();
    settings.bus.field='base_kv';
    settings.bus.min=100;
    settings.bus.max=140;
    settings.bus.low='#000000';
    settings.bus.high='#ffffff';
    const result=createColorPresentation(makeSnapshot(),settings);
    expect(result.nodeColors['bus:1']).toBe('#f2f2f2');
    expect(result.nodeColors['bus:3']).toBe('#000000');
    expect(result.legends[0].gradient).toEqual({low:'#000000',high:'#ffffff',min:100,max:140});
  });

  it('uses a deterministic midpoint for a constant numeric domain and colors missing values',()=>{
    const data=makeSnapshot();
    data.case.buses=data.case.buses.map(bus=>({...bus,base_kv:100}));
    data.graph.nodes=data.graph.nodes.map(node=>({...node,base_kv:100}));
    const settings=defaultColorSettings();
    settings.bus.field='base_kv';
    settings.bus.low='#000000';
    settings.bus.high='#ffffff';
    const result=createColorPresentation(data,settings);
    expect(result.nodeColors['bus:1']).toBe('#808080');
    expect(result.legends[0].gradient?.min).toBe(100);
    expect(result.legends[0].gradient?.max).toBe(100);

    settings.bus.field='voltage_pu';
    expect(createColorPresentation(makeSnapshot(),settings).nodeColors['bus:1']).toBe('#8b5cf6');
  });

  it('handles numeric domains larger than the JavaScript argument limit',()=>{
    const data=makeSnapshot();
    data.case.buses=Array.from({length:150_000},(_,index)=>({
      number:index+1,name:`Bus ${index+1}`,base_kv:100,bus_type:1,area:1,zone:1,voltage_pu:1,source:source(index+1),
    }));
    const settings=defaultColorSettings();
    settings.bus.field='number';
    const result=createColorPresentation(data,settings);
    expect(result.legends[0].gradient).toMatchObject({min:1,max:150_000});
  });

  it('uses equipment fields for edges and leaves synthetic junctions white',()=>{
    const settings=defaultColorSettings();
    settings.equipment.field='kind';
    settings.equipment.colors.line='#123456';
    settings.equipment.colors.switch='#654321';
    const result=createColorPresentation(makeSnapshot(),settings);
    expect(result.edgeColors).toEqual({e1:'#123456',e2:'#654321'});
    expect(result.nodeColors).toEqual({});
    expect(result.legends[0].target).toBe('equipment');
  });

  it('ignores invalid colors and safely loads and saves preferences without case data',()=>{
    const values=new Map<string,string>();
    const storage={getItem:(key:string)=>values.get(key)??null,setItem:(key:string,value:string)=>{values.set(key,value);}};
    values.set('nodal-field-colors',JSON.stringify({bus:{field:'area',palette:'categorical-8',colors:JSON.parse('{"__proto__":"#112233","1":"red"}'),low:'#123456',high:'#abcdef',missing:'#010203',mixed:'#040506',min:null,max:null},equipment:{field:'unknown',palette:'bad',colors:{},low:'bad',high:'bad',missing:'bad',mixed:'bad',min:null,max:null}}));
    const loaded=loadColorSettings(storage);
    expect(loaded.bus.field).toBe('area');
    expect(loaded.bus.colors).toEqual({});
    expect(loaded.equipment.field).toBe('');
    saveColorSettings(loaded,storage);
    const serialized=values.get('nodal-field-colors')??'';
    expect(serialized).not.toContain('Alpha');
    expect(JSON.parse(serialized).bus.colors).toEqual({});
  });

  it('keeps custom text colors for the session but omits them from saved preferences',()=>{
    const values=new Map<string,string>();
    const storage={getItem:(key:string)=>values.get(key)??null,setItem:(key:string,value:string)=>{values.set(key,value);}};
    const settings=defaultColorSettings();
    settings.bus.field='name';
    settings.bus.palette='categorical-10';
    settings.bus.colors.Alpha='#123456';
    settings.equipment.field='circuit';
    settings.equipment.colors['1']='#654321';
    saveColorSettings(settings,storage);
    expect(settings.bus.colors).toEqual({Alpha:'#123456'});
    expect(settings.equipment.colors).toEqual({'1':'#654321'});
    const serialized=values.get('nodal-field-colors')??'';
    expect(serialized).not.toContain('Alpha');
    expect(serialized).not.toContain('#123456');
    expect(serialized).not.toContain('#654321');
    const reloaded=loadColorSettings(storage);
    expect(reloaded.bus).toMatchObject({field:'name',palette:'categorical-10',colors:{}});
    expect(reloaded.equipment).toMatchObject({field:'circuit',colors:{}});
  });

  it('discards both stored bounds when the minimum exceeds the maximum',()=>{
    const value={...defaultColorSettings(),bus:{...defaultColorSettings().bus,field:'base_kv',min:500,max:100}};
    const loaded=loadColorSettings({getItem:()=>JSON.stringify(value)});
    expect(loaded.bus.min).toBeNull();
    expect(loaded.bus.max).toBeNull();
  });

  it('keeps one-sided bounds ordered when they pass the automatic domain',()=>{
    const minSettings=defaultColorSettings();
    minSettings.bus.field='voltage_pu';
    minSettings.bus.min=2;
    minSettings.bus.low='#000000';
    minSettings.bus.high='#ffffff';
    const minResult=createColorPresentation(makeSnapshot(),minSettings);
    expect(minResult.legends[0].gradient).toMatchObject({min:2,max:2});
    expect(minResult.nodeColors['bus:3']).toBe('#000000');

    const maxSettings=defaultColorSettings();
    maxSettings.bus.field='base_kv';
    maxSettings.bus.max=50;
    maxSettings.bus.low='#000000';
    maxSettings.bus.high='#ffffff';
    const maxResult=createColorPresentation(makeSnapshot(),maxSettings);
    expect(maxResult.legends[0].gradient).toMatchObject({min:50,max:50});
    expect(maxResult.nodeColors['bus:3']).toBe('#ffffff');
  });

  it('survives corrupt and unavailable storage',()=>{
    expect(loadColorSettings({getItem:()=>'{bad'})).toEqual(defaultColorSettings());
    expect(loadColorSettings({getItem:()=>{throw new Error('denied');}})).toEqual(defaultColorSettings());
    expect(()=>saveColorSettings(defaultColorSettings(),{setItem:()=>{throw new Error('denied');}})).not.toThrow();
  });
});
