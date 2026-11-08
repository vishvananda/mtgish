/* Run against the generated standalone file, including the actual Wasm worker. */
const assert = require('node:assert/strict');
const {pathToFileURL} = require('node:url');
const path = require('node:path');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
let browser;
(async () => {
  const input=process.argv[2] || '__build/card-blocks.html';
  const target=/^https?:/.test(input)?input:pathToFileURL(path.resolve(input)).href;
  browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
  const page=await browser.newPage({viewport:{width:1580,height:1000},acceptDownloads:true});
  const errors=[],network=[];
  page.on('pageerror',e=>errors.push(e.message));
  page.on('request',r=>{if(/^https?:/.test(r.url()))network.push(r.url());});
  await page.goto(target,{timeout:120000});
  await page.waitForFunction(()=>window.cardBlocks?.ready() && window.cardBlocks.valid(),{timeout:120000});
  assert.equal(await page.locator('#card-title').textContent(),'Lightning Bolt');
  await page.evaluate(async()=>{window.cardBlocks.workspace.render();await Blockly.renderManagement.finishQueuedRenders();window.cardBlocks.workspace.zoomToFit();});
  await page.screenshot({path:process.env.BLOCKLY_SCREENSHOT || '/tmp/card-blocks.png',fullPage:true});
  const names=['Cloudshift',"Conjurer's Closet",'Restoration Angel','Basalt Golem','Counterspell','Llanowar Elves','Ambergris, Agent of Progress','Ambergris, Citadel Agent','Fire // Ice','Delver of Secrets // Insectile Aberration','Jace, the Mind Sculptor','The Wandering Emperor','Brainstorm','Halfdane'];
  const roundtrips=await page.evaluate(async names=>{
    const d=window.cardBlocks,out=[];
    const stable=v=>Array.isArray(v)?v.map(stable):v&&typeof v==='object'?Object.fromEntries(Object.keys(v).sort().map(k=>[k,stable(v[k])])):v;
    for(const name of names){
      const e=d.payload.cards.find(c=>c.name===name || c.name.split(' // ').sort().join(' // ')===name.split(' // ').sort().join(' // '));if(!e)throw Error('Missing catalog entry '+name);
      const value=(await d.shard(e.shard))[e.index];d.loadIR(value,false);
      if(d.encode(stable(d.generated()))!==d.encode(stable(value)))throw Error('IR/block roundtrip mismatch: '+name);
      out.push({name,blocks:d.workspace.getAllBlocks(false).length});
    }
    return out;
  },names);
  // Search UI, including Enter-to-load and names containing apostrophes.
  await page.locator('#search').fill("Conjurer's Closet");await page.locator('#search').press('Enter');
  await page.waitForFunction(()=>document.getElementById('card-title').textContent==="Conjurer's Closet" && window.cardBlocks.valid());
  // Build a card, then edit the actual numeric block field.
  await page.locator('#starter').selectOption('draw');await page.locator('#new-card').click();
  await page.waitForFunction(()=>window.cardBlocks.valid() && document.getElementById('card-title').textContent==='New draw spell');
  await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='Expr'&&b.state.variant==='Constant').getField('_ARG_0').setValue('2'));
  await page.waitForFunction(()=>window.cardBlocks.valid()?.includes('"args": 2'));
  const downloadPromise=page.waitForEvent('download');await page.locator('#export').click();const download=await downloadPromise;
  assert.equal(download.suggestedFilename(),'New_draw_spell.ir.json');
  const downloaded=JSON.parse(require('node:fs').readFileSync(await download.path(),'utf8'));
  assert.equal(downloaded.faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount.args,2);
  // Invalid scope: well-typed BoundValue but no preceding binding.
  await page.evaluate(()=>{const d=window.cardBlocks,v=d.blank(true);v.faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount={kind:'BoundValue',args:42};d.loadIR(v,false);});
  await page.waitForFunction(()=>document.getElementById('validation').classList.contains('invalid'));
  assert.match(await page.locator('#validation-detail').textContent(),/bound value|before.*set|result/i);
  assert.equal(await page.locator('#export').isDisabled(),true);
  // Browser and Rust both preserve all digits of the largest i64.
  await page.evaluate(()=>{const d=window.cardBlocks,v=d.blank(true);v.faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount.args=9223372036854775807n;d.loadIR(v,false);});
  await page.waitForFunction(()=>window.cardBlocks.valid()?.includes('9223372036854775807'));
  assert.match(await page.locator('#ir').textContent(),/9223372036854775807/);
  // Workspace serialization, type compatibility and live list mutations.
  await page.evaluate(()=>{const d=window.cardBlocks;d.loadIR(d.blank(),false);const saved=d.savedWorkspace(),expected=d.encode(d.generated());d.restore(saved);if(d.encode(d.generated())!==expected)throw Error('Workspace save/restore changed IR');});
  await page.waitForFunction(()=>window.cardBlocks.valid());
  await page.evaluate(()=>{
    const d=window.cardBlocks,list=d.workspace.getAllBlocks(false).find(b=>b.state.ty.vec?.named==='Ability');list.resize(1);
    if(d.generated().faces[0].abilities.length!==1)throw Error('List plus did not add an ability');
  });
  await page.waitForFunction(()=>window.cardBlocks.valid());
  await page.locator('#undo').click();
  await page.waitForFunction(()=>window.cardBlocks.generated().faces[0].abilities.length===0 && window.cardBlocks.valid());
  await page.locator('#redo').click();
  await page.waitForFunction(()=>window.cardBlocks.generated().faces[0].abilities.length===1 && window.cardBlocks.valid());
  await page.evaluate(()=>{
    const d=window.cardBlocks,list=d.workspace.getAllBlocks(false).find(b=>b.state.ty.vec?.named==='Instruction');list.resize(1);
    const op=d.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='Operation'&&b.state.variant==='Draw');
    const expr=Blockly.serialization.blocks.append(d.blockState({named:'Expr'},{kind:'Constant',args:1}),d.workspace);
    const players=op.getInputTargetBlock('F_players'),socket=op.getInput('F_players').connection;
    if(d.workspace.connectionChecker.doTypeChecks(expr.outputConnection,socket))throw Error('An Expr was accepted in a PlayerSet socket');
    if(!d.workspace.connectionChecker.doTypeChecks(players.outputConnection,socket))throw Error('A PlayerSet was rejected in its own socket');
    expr.dispose(false);
  });
  // Finish on a normal card for the screenshot / resumed workspace.
  await page.evaluate(async()=>{const d=window.cardBlocks;await d.loadCard(d.payload.cards.find(c=>c.name==='Cloudshift'));});
  await page.waitForFunction(()=>window.cardBlocks.valid() && document.getElementById('card-title').textContent==='Cloudshift');
  const unexpected=network.filter(url=>url!==target && (!/^https?:/.test(target)||url!==new URL('/favicon.ico',target).href));
  assert.deepEqual(unexpected,[],'The demo fetched a runtime dependency');
  assert.deepEqual(errors,[],'Browser errors');
  console.log(JSON.stringify({offline:target.startsWith('file:'),noExternalAssets:true,wasm:true,cardRoundtrips:roundtrips,editing:true,download:true,invalidScopeRejected:true,exactI64:true,workspaceRoundtrip:true,errors},null,2));
  await browser.close();
})().catch(async e=>{console.error(e);if(browser)await browser.close();process.exitCode=1;});
