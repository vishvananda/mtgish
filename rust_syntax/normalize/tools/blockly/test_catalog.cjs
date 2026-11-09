/* Full-corpus typed block serialization + the actual Rust/Wasm validator. */
const assert=require('node:assert/strict');
const {pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE || 'playwright');
let browser;
(async()=>{
 browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 const page=await browser.newPage();
 page.on('console',m=>{if(m.text().startsWith('Catalog:'))console.log(m.text());});
 await page.goto(pathToFileURL(require('node:path').resolve(process.argv[2]||'__build/card-blocks.html')).href,{timeout:120000});
 await page.waitForFunction(()=>window.cardBlocks?.ready()&&window.cardBlocks.valid(),null,{timeout:120000});
 const report=await page.evaluate(async()=>{
  const d=window.cardBlocks,report={expected:d.payload.record_count,records:0,blockRoundtrips:0,bindingPortCards:0,wasmValid:0,failures:[]};
  const stable=v=>Array.isArray(v)?v.map(stable):v&&typeof v==='object'?Object.fromEntries(Object.keys(v).sort().map(k=>[k,stable(v[k])])):v;
  const fake=s=>({type:s.type,state:s.extraState,isEnabled:()=>true,getFieldValue:n=>{const v=s.fields?.[n],p=d.parts(s.extraState).find(p=>'_'+p.key===n);return p?.ty.named==='bool'?(v===true||v==='TRUE'):v;},getInputTargetBlock:n=>{const child=s.inputs?.[n]?.block||s.inputs?.[n]?.shadow;return child?fake(child):null;},getNextBlock:()=>s.next?.block?fake(s.next.block):null});
  // Walk wire types independently of the block presentation's wrapper mapping.
  const idKinds={Slot:'Target',ResultId:'Result',Binding:'Loop',SelectionId:'Selection'};
  const pointer=(path,key)=>path+'/'+String(key).replace(/~/g,'~0').replace(/\//g,'~1');
  function wireSites(ty,value,path='',out=[]) {
   if(idKinds[ty.named]){out.push([path,idKinds[ty.named],value]);return out;}
   if(ty.vec){value.forEach((v,i)=>wireSites(ty.vec,v,pointer(path,i),out));return out;}
   if(ty.option){if(value!==null)wireSites(ty.option,value,path,out);return out;}
   if(ty.tuple){ty.tuple.forEach((t,i)=>wireSites(t,value[i],pointer(path,i),out));return out;}
   const type=d.types[ty.named];if(!type)return out;let fields=type.fields,args=value,base=path;
   if(type.kind==='enum'){const name=type.adjacent?value.kind:typeof value==='string'?value:Object.keys(value)[0],variant=type.variants.find(v=>v.wire===name);fields=variant.fields;args=type.adjacent?value.args:value[name];base=pointer(path,type.adjacent?'args':name);}
   if(fields.kind==='record')for(const field of fields.fields)wireSites(field.ty,args[field.name],pointer(base,field.name),out);
   if(fields.kind==='tuple')fields.items.forEach((t,i)=>wireSites(t,fields.items.length===1?args:args[i],fields.items.length===1?base:pointer(base,i),out));
   return out;
  }
  for(let n=0;n<d.payload.shards.length;n++){
   const records=await d.shard(n);
   for(const value of records){
    report.records++;
    try{
     const state=d.blockState({named:'UnifiedCard'},value),back=d.fromBlock(fake(state));
     if(d.encode(stable(value))!==d.encode(stable(back)))throw Error('Model changed during block roundtrip');
     report.blockRoundtrips++;
     const expected=wireSites({named:'UnifiedCard'},value).sort((a,b)=>a[0].localeCompare(b[0]));
     const ports=d.bindings.ports(fake(state),value).map(p=>[p.path,p.kind,p.id]).sort((a,b)=>a[0].localeCompare(b[0]));
     if(d.encode(expected)!==d.encode(ports))throw Error('Binding port paths/types/IDs differ from wire types: '+JSON.stringify({expected,ports}));
     report.bindingPortCards++;
    }catch(e){report.failures.push({name:value.faces.map(f=>f.name).join(' // ')||value.layout,phase:'blocks',error:e.message});}
   }
   const checked=await d.validateTexts(records.map(value=>d.encode(value)));
   checked.forEach((result,i)=>{if(result.valid)report.wasmValid++;else report.failures.push({name:records[i].faces.map(f=>f.name).join(' // ')||records[i].layout,phase:'wasm',error:result.error});});
   if(n%100===0)console.log(`Catalog: ${report.records} records, ${report.failures.length} failures`);
  }
  return report;
 });
 console.log(JSON.stringify(report,null,2));
 assert.ok(Number.isInteger(report.expected));assert.equal(report.records,report.expected);assert.equal(report.blockRoundtrips,report.records);assert.equal(report.wasmValid,report.records);assert.equal(report.bindingPortCards,report.records);assert.deepEqual(report.failures,[]);
 await browser.close();
})().catch(async e=>{console.error(e);if(browser)await browser.close();process.exitCode=1;});
