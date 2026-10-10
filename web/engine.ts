import type {WorkerRequest,WorkerResponse,Snapshot,Path} from './types';

export class BrowserEngine {
  private worker:Worker|null=null;
  private sequence=0;
  private pending=new Map<number,{resolve:(response:WorkerResponse)=>void;reject:(error:unknown)=>void}>();

  private reset() {
    this.worker?.terminate();
    for (const request of this.pending.values()) request.reject(new DOMException('Replaced by a new file','AbortError'));
    this.pending.clear();
    this.worker=new Worker(new URL('./worker.ts',import.meta.url),{type:'module'});
    this.worker.onmessage=(event:MessageEvent<WorkerResponse>)=>{
      const response=event.data;
      const request=this.pending.get(response.id);
      if (!request) return;
      this.pending.delete(response.id);
      if (response.type==='error') request.reject(response.error);
      else request.resolve(response);
    };
    this.worker.onerror=(event)=>{
      for (const request of this.pending.values()) request.reject(new Error(event.message||'Unable to start the analysis worker.'));
      this.pending.clear();
    };
  }

  private send(request:WorkerRequest,transfer:Transferable[]=[]):Promise<WorkerResponse> {
    return new Promise((resolve,reject)=>{
      if (!this.worker) {reject(new Error('Open a RAW file first.'));return;}
      this.pending.set(request.id,{resolve,reject});
      this.worker.postMessage(request,transfer);
    });
  }

  async load(bytes:ArrayBuffer):Promise<Snapshot> {
    this.reset();
    const response=await this.send({id:++this.sequence,type:'load',bytes},[bytes]);
    if (response.type!=='loaded') throw new Error('Unexpected worker response');
    return response.snapshot;
  }

  async path(from:number,to:number):Promise<Path|null> {
    const response=await this.send({id:++this.sequence,type:'path',from,to});
    if (response.type!=='path') throw new Error('Unexpected worker response');
    return response.path;
  }

  dispose() {this.worker?.terminate();this.worker=null;for(const request of this.pending.values()) request.reject(new DOMException('Analysis closed','AbortError'));this.pending.clear();}
}
