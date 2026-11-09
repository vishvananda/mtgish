/* Edit the actual Rust card-code view, search blocks, export and migrate a saved workspace. */
const assert=require('node:assert/strict');
const {pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
let browser,page;
(async()=>{
 const input=process.argv[2]||'__build/card-blocks.html';
 browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 page=await browser.newPage({viewport:{width:1600,height:1050},acceptDownloads:true});page.setDefaultTimeout(30000);
 const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(/^https?:/.test(input)?input:pathToFileURL(require('node:path').resolve(input)).href,{timeout:120000});
 await page.waitForFunction(()=>window.cardBlocks?.valid());
 await page.locator('#code-view').click();await page.locator('#dump-format').selectOption('human');
 await page.waitForFunction(()=>document.getElementById('human-editor').value.includes('Lightning Bolt'));
 const code=await page.locator('#human-editor').inputValue();
 assert.equal(await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.pr==='single').getInput('ROOT_schema').isVisible()),false);
 assert.match(code,/AnyTarget/);assert.doesNotMatch(code,/result:|schema_version:|preventable:|Effect\s*\{/);
 await page.locator('#human-editor').fill(code.replace('amount: 3','amount: 5'));
 assert.equal(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount.args),3);
 await page.locator('#apply-code').click();
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount.args===5);
 await page.waitForFunction(()=>document.getElementById('human-editor').value.includes('amount: 5'));
 // A named target's domain changes both its selector and the consuming recipient reference.
 await page.locator('#human-editor').fill((await page.locator('#human-editor').inputValue()).replace('AnyTarget','Creature'));
 await page.locator('#apply-code').click();
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.announcement.targets[0].kind.Object);
 assert.equal(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.recipients.kind),'Objects');
 const before=await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated()));
 await page.waitForFunction(()=>document.getElementById('human-editor').value.includes('Creature'));
 const good=await page.locator('#human-editor').inputValue();
 await page.locator('#human-editor').fill(good.replace('amount: 5','ammount: 5'));await page.locator('#apply-code').click();
 await page.waitForFunction(()=>/missing|unknown/.test(document.getElementById('code-status').textContent));
 assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),before);
 await page.locator('#human-editor').fill(good);await page.locator('#apply-code').click();await page.waitForFunction(()=>window.cardBlocks.valid()&&document.getElementById('code-status').textContent.startsWith('Checked by Rust'));
 const downloading=page.waitForEvent('download');await page.locator('#export-code').click();const download=await downloading;
 assert.match(download.suggestedFilename(),/\.card$/);
 const text=require('node:fs').readFileSync(await download.path(),'utf8');
 assert.equal((await page.evaluate(text=>window.cardBlocks.parseCode(text),text)).valid,true);
 // Category membership and labels are exported from the shared Rust registry.
 const metadata=await page.evaluate(()=>window.cardBlocks.schema.authoring);
 assert.equal(metadata.actions.Draw.name,'DrawCards');assert.equal(metadata.actions.DealDamage.category,'Damage & life');
 assert.equal(metadata.references.PlayerRef.EventSubject,'EventPlayer');assert.equal(metadata.references.ObjectRef.EventSubject,'EventObject');
 await page.locator('#block-search').fill('damage');
 await page.waitForFunction(()=>window.cardBlocks.workspace.getFlyout().getWorkspace().getAllBlocks(false).some(b=>b.state.op==='DealDamage'));
 await page.locator('#block-search').fill('');
 // Old UI fingerprints at the same frozen IR schema migrate through the typed codec.
 await page.evaluate(()=>{const d=window.cardBlocks,s=d.savedWorkspace(),before=d.encode(d.generated());s.model='previous-ui-model';d.restore(s);if(d.encode(d.generated())!==before)throw Error('Workspace migration changed IR');});
 await page.waitForFunction(()=>window.cardBlocks.valid());
 const actionCount=await page.evaluate(()=>{
  const d=window.cardBlocks;let count=0;
  for(const v of d.schema.types.Operation.variants){
   if(v.name.startsWith('Draft')||['Named','Primitive','Form'].includes(v.name))continue;
   d.blockState({named:'Instruction'},d.actionValue(v.wire));
   if(d.schema.authoring.actions[v.wire].category==='Producer forms')throw Error('Uncategorized action '+v.name);
   count++;
  }
  return count;
 });
 assert.ok(actionCount>=134);
 const wasmExamples=await page.evaluate(async()=>{
  const d=window.cardBlocks,names=['Cloudshift',"Conjurer's Closet",'Restoration Angel','Basalt Golem','Counterspell','Llanowar Elves','Fire // Ice','Delver of Secrets // Insectile Aberration','Jace, the Mind Sculptor','The Wandering Emperor','Brainstorm','Halfdane','Rolling Hamsphere','Knowledge Pool'];
  for(const name of names){
   const entry=d.payload.cards.find(c=>c.name===name||c.name.split(' // ').sort().join(' // ')===name.split(' // ').sort().join(' // '));
   if(!entry)throw Error('Missing '+name);
   const card=(await d.shard(entry.shard))[entry.index],formatted=await d.formatCode(d.encode(card));
   if(!formatted.valid)throw Error(name+': '+formatted.error);
   const parsed=await d.parseCode(formatted.code);if(!parsed.valid)throw Error(name+': '+parsed.error);
   const again=await d.formatCode(parsed.ir);if(again.code!==formatted.code)throw Error(name+': unstable card code');
  }
  return names.length;
 });
 const malformed=[
  'Card{name:"Counter entry",properties:{card_types:[Instant]},abilities:[Spell{do:[CounterWith{object:This,destination:OwnersHand,entry:[Objects(Targets(t99))]}]}]}',
  'Card{name:"Entry rules",properties:{card_types:[Instant]},abilities:[Spell{do:[MoveObjects{objects:One(This),destination:Exile,entry:{states:[RulesUntil{rules:[Objects(Targets(t99))],until:EndOfTurn}]}}]}]}',
  ...['Between{minimum:-1,maximum:1}','Between{minimum:3,maximum:1}'].map(count=>'Card{name:"Bounds",properties:{card_types:[Instant]},abilities:[Spell{targets:[Target{name:t0,select:Creature,count:'+count+'}],do:[Reveal{objects:Targets(t0)}]}]}')
 ];
 for(const code of malformed)assert.equal((await page.evaluate(code=>window.cardBlocks.parseCode(code),code)).valid,false);
 assert.deepEqual(errors,[]);
 await page.screenshot({path:process.env.BLOCKLY_SCREENSHOT||'/tmp/card-code.png',fullPage:true});
 console.log(JSON.stringify({codeEditing:true,validation:true,typedTargetReferences:true,typosRejected:true,draftPreserved:true,export:true,search:true,sharedNaming:true,savedWorkspaceMigration:true,wasmExamples,actionCount,malformedInputsRejected:malformed.length,errors},null,2));
 await browser.close();
})().catch(async e=>{console.error(e);if(page){await page.screenshot({path:'/tmp/card-code-failure.png'});console.error(await page.locator('#code-status').textContent());}if(browser)await browser.close();process.exitCode=1;});
