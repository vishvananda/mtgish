/* Upgrade a real old workspace before the Wasm worker has started. */
const assert=require('node:assert/strict'),{pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
let browser;
(async()=>{
 const input=process.argv[2]||'__build/card-blocks.html',url=/^https?:/.test(input)?input:pathToFileURL(require('node:path').resolve(input)).href;
 const old=require('./fixtures/workspace-v4-schema179.json');
 browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 async function open(saved){
  const context=await browser.newContext();await context.addInitScript(saved=>{localStorage.setItem('mtgish.blockly.v1',JSON.stringify(saved));},saved);
  const page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(url,{timeout:120000});await page.waitForFunction(()=>window.cardBlocks?.valid(),null,{timeout:120000});return {context,page,errors};
 }
 const good=await open(old.workspace),page=good.page;
 await page.waitForFunction(()=>document.getElementById('card-title').textContent==='Desertion');
 const state=await page.evaluate(()=>{
  const d=window.cardBlocks,ir=d.generated(),spell=ir.faces[0].abilities.find(a=>a.kind==='Spell');
  const operations=spell.args.body.instructions.map(i=>i.args.operation),counter=operations.find(o=>o.kind==='CounterWith');
  return {schema:ir.schema_version,destination:counter.args.destination,
   saved:JSON.parse(localStorage.getItem('mtgish.blockly.v1')),
   backup:JSON.parse(localStorage.getItem('mtgish.blockly.before-schema-upgrade'))};
 });
 assert.ok(state.schema>179);assert.equal(state.saved.schema,state.schema);assert.equal(state.backup.schema,179);
 assert.equal(state.destination.Battlefield.controller.Player.kind,'AbilityController');assert.equal(state.destination.Battlefield.arrival.tapped,false);
 // Exact IR import rejects unknown fields before changing the workspace.
 const before=await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated()));
 await page.locator('#file').setInputFiles({name:'invalid.ir.json',mimeType:'application/json',buffer:Buffer.from(before.slice(0,-1)+',"unexpected":true}')});
 await page.waitForFunction(()=>document.getElementById('notice').textContent.includes('unknown'));
 assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),before);
 // The migration bridge accepts the ordinary validator's supported depth.
 const depth=await page.evaluate(async()=>{
  const d=window.cardBlocks,ir=d.blank(true);
  let amount=ir.faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount;
  for(let i=0;i<65;i++)amount={kind:'Add',args:[amount,{kind:'Constant',args:0}]};
  ir.faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount=amount;
  return {good:await d.migrateIR(d.encode(ir)),bad:await d.migrateIR('['.repeat(256)+']'.repeat(256))};
 });
 assert.equal(depth.good.valid,true,depth.good.error);assert.equal(depth.bad.valid,false);assert.match(depth.bad.error,/255 levels/);
 // A synchronous block edit must win over an in-flight upgrade response.
 const raced=await page.evaluate(async old=>{
  const d=window.cardBlocks,pending=d.migrateWorkspace(old).then(()=>null,e=>e.message);
  d.workspace.getTopBlocks(false)[0].setFieldValue('My newer draft','_F_name');
  return {error:await pending,name:d.generated().faces[0].name};
 },old.workspace);
 assert.match(raced.error,/changed while upgrading/);assert.equal(raced.name,'My newer draft');
 assert.deepEqual(good.errors,[]);await good.context.close();
 // An incomplete old draft must remain recoverable and must not be overwritten
 // by the default card after upgrade fails.
 const broken=structuredClone(old.workspace);delete broken.workspace.blocks.blocks[0].fields._F_name;
 const failed=await open(broken);
 const retained=await failed.page.evaluate(()=>({save:JSON.parse(localStorage.getItem('mtgish.blockly.v1')),backup:JSON.parse(localStorage.getItem('mtgish.blockly.before-schema-upgrade')),meta:document.getElementById('workspace-meta').textContent}));
 assert.deepEqual(retained.save,broken);assert.deepEqual(retained.backup,broken);assert.match(retained.meta,/preserved/);
 await failed.page.locator('#new-card').click();await failed.page.waitForFunction(()=>window.cardBlocks.valid()&&JSON.parse(localStorage.getItem('mtgish.blockly.v1')).schema===window.cardBlocks.schema.version);
 assert.deepEqual(failed.errors,[]);await failed.context.close();
 console.log(JSON.stringify({startupUpgrade:true,typedEntry:true,backup:true,strictImport:true,failedDraftPreserved:true,concurrentEditPreserved:true,supportedDepth:true,explicitNewCard:true,errors:[]},null,2));
 await browser.close();
})().catch(async e=>{console.error(e);if(browser)await browser.close();process.exitCode=1});
