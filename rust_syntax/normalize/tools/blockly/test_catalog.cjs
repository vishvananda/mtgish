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
  const d=window.cardBlocks,report={expected:d.payload.record_count,records:0,blockRoundtrips:0,wasmValid:0,failures:[]};
  const stable=v=>Array.isArray(v)?v.map(stable):v&&typeof v==='object'?Object.fromEntries(Object.keys(v).sort().map(k=>[k,stable(v[k])])):v;
  const fake=s=>({type:s.type,state:s.extraState,isEnabled:()=>true,getFieldValue:n=>s.fields?.[n],getInputTargetBlock:n=>s.inputs?.[n]?.block?fake(s.inputs[n].block):null});
  for(let n=0;n<d.payload.shards.length;n++){
   const records=await d.shard(n);
   for(const value of records){
    report.records++;
    try{
     const state=d.blockState({named:'UnifiedCard'},value),back=d.fromBlock(fake(state));
     if(d.encode(stable(value))!==d.encode(stable(back)))throw Error('Model changed during block roundtrip');
     report.blockRoundtrips++;
    }catch(e){report.failures.push({name:value.faces.map(f=>f.name).join(' // ')||value.layout,phase:'blocks',error:e.message});}
   }
   const checked=await d.validateTexts(records.map(value=>d.encode(value)));
   checked.forEach((result,i)=>{if(result.valid)report.wasmValid++;else report.failures.push({name:records[i].faces.map(f=>f.name).join(' // ')||records[i].layout,phase:'wasm',error:result.error});});
   if(n%100===0)console.log(`Catalog: ${report.records} records, ${report.failures.length} failures`);
  }
  return report;
 });
 console.log(JSON.stringify(report,null,2));
 assert.ok(Number.isInteger(report.expected));assert.equal(report.records,report.expected);assert.equal(report.blockRoundtrips,report.records);assert.equal(report.wasmValid,report.records);assert.deepEqual(report.failures,[]);
 await browser.close();
})().catch(async e=>{console.error(e);if(browser)await browser.close();process.exitCode=1;});
