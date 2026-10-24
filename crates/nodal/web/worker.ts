/// <reference lib="webworker" />
import init, {parse_raw, type NodalCase} from './wasm/nodal';
import type {Diagnostic,WorkerRequest,WorkerResponse,Snapshot,Path} from './types';

const scope=self as unknown as DedicatedWorkerGlobalScope;
const ready=init();
let current:NodalCase|null=null;
let queue=Promise.resolve();
scope.onmessage=(event:MessageEvent<WorkerRequest>)=>{
  const request=event.data;
  queue=queue.then(async()=>{
    try {
      await ready;
      let response:WorkerResponse;
      if (request.type==='load') {
        current?.free();current=null;
        current=parse_raw(new Uint8Array(request.bytes));
        current.set_merge_zero_impedance(request.mergeZeroImpedance);
        response={id:request.id,type:'loaded',snapshot:current.snapshot() as Snapshot};
      } else if (request.type==='topology') {
        if (!current) throw new Error('Open a RAW file before changing topology.');
        current.set_merge_zero_impedance(request.mergeZeroImpedance);
        response={id:request.id,type:'loaded',snapshot:current.snapshot() as Snapshot};
      } else {
        if (!current) throw new Error('Open a RAW file before finding a path.');
        response={id:request.id,type:'path',path:current.shortest_path(request.from,request.to) as Path|null};
      }
      scope.postMessage(response);
    } catch (error) {
      const detail=error as Partial<Diagnostic>;
      const diagnostic:Diagnostic={severity:'error',section:detail?.section??'APPLICATION',line:detail?.line??0,message:detail?.message??String(error)};
      scope.postMessage({id:request.id,type:'error',error:diagnostic} satisfies WorkerResponse);
    }
  });
};
