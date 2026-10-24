import {invoke,isTauri} from '@tauri-apps/api/core';

export async function saveExport(data:string,mimeType:string,filename:string):Promise<void> {
  if(isTauri()) {
    await invoke<void>('save_export',{data,filename});
    return;
  }

  const url=URL.createObjectURL(new Blob([data],{type:mimeType}));
  try {
    const link=document.createElement('a');
    link.href=url;
    link.download=filename;
    link.click();
  } finally {
    setTimeout(()=>URL.revokeObjectURL(url),1000);
  }
}
