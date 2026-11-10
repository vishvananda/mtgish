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
 // Control instructions are discoverable through the same shared search as
 // actions. Build a real instruction sequence from its flyout starter, keeping
 // the existing action and receipt inside it.
 for(const wire of ['Group','If','Optional','ChooseOne','ForEachPlayer','ForEachObject','Repeatable','Times']) {
  await page.locator('#block-search').fill(wire);
  await page.waitForFunction(wire=>window.cardBlocks.workspace.getFlyout().getWorkspace().getTopBlocks(false).some(b=>b.state.ty.named==='Instruction'&&b.state.variant===wire),wire);
 }
 await page.locator('#block-search').fill('choose an option');
 assert.equal(await page.evaluate(()=>{const d=window.cardBlocks,b=d.workspace.getFlyout().getWorkspace().getTopBlocks(false).find(b=>b.state.variant==='ChooseOne');return d.fromBlock(b).args.options.length;}),2);
 await page.locator('#block-search').fill('results & receipts');
 assert.deepEqual(await page.evaluate(()=>window.cardBlocks.workspace.getFlyout().getWorkspace().getTopBlocks(false).map(b=>b.state.op).sort()),['BindValue','JoinResults','NoteObjects']);
 await page.locator('#block-search').fill('sequence of instructions');
 const receiptBefore=await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.result);
 await page.evaluate(()=>{
  const d=window.cardBlocks,w=d.workspace,starter=w.getFlyout().getWorkspace().getTopBlocks(false).find(b=>b.state.variant==='Group'),saved=Blockly.serialization.blocks.save(starter),group=Blockly.serialization.blocks.append(saved,w),ability=w.getAllBlocks(false).find(b=>b.state.ty.named==='Ability'&&b.state.variant==='Spell'),body=ability.getInputTargetBlock('F_body');
  body.previousConnection.disconnect();ability.getInput('F_body').connection.connect(group.previousConnection);
  const sequence=d.parts(group.state).find(p=>p.flow);group.getInput(sequence.key).connection.connect(body.previousConnection);
 });
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].kind==='Group');
 assert.equal(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.instructions[0].args.result),receiptBefore);
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
  const d=window.cardBlocks,names=['Cloudshift',"Conjurer's Closet",'Restoration Angel','Basalt Golem','Counterspell','Llanowar Elves','Fire // Ice','Delver of Secrets // Insectile Aberration','Jace, the Mind Sculptor','The Wandering Emperor','Brainstorm','Halfdane','Rolling Hamsphere','Knowledge Pool','Lagrella, the Magpie','Two-Headed Giant','Mana Clash'];
  for(const name of names){
   const entry=d.payload.cards.find(c=>c.name===name||c.name.split(' // ').sort().join(' // ')===name.split(' // ').sort().join(' // '));
   if(!entry)throw Error('Missing '+name);
   const card=(await d.shard(entry.shard))[entry.index],formatted=await d.formatCode(d.encode(card));
   if(!formatted.valid)throw Error(name+': '+formatted.error);
   if(name==='Two-Headed Giant'){if((formatted.code.match(/CountCoinOutcomes/g)||[]).length!==2||formatted.code.includes('CoinFlipsWon'))throw Error('Coin faces became called wins');}
   if(name==='Mana Clash'&&(formatted.code.includes('CoinFlipsWon')||!formatted.code.includes('Heads')||!formatted.code.includes('CoinCameUp')))throw Error('Collective coin faces became a win/loss tally');
   const parsed=await d.parseCode(formatted.code);if(!parsed.valid)throw Error(name+': '+parsed.error);
   const again=await d.formatCode(parsed.ir);if(again.code!==formatted.code)throw Error(name+': unstable card code');
  }
  return names.length;
 });
 const malformed=[
  'Card{name:"Counter entry",properties:{card_types:[Instant]},abilities:[Spell{do:[CounterWith{object:This,destination:OwnersHand,entry:[Objects(Targets(t99))]}]}]}',
  'Card{name:"Entry rules",properties:{card_types:[Instant]},abilities:[Spell{do:[MoveObjects{objects:One(This),destination:Exile,entry:{states:[ChangesUntil{changes:[],duration:WhileItMatches(InSet(Targets(t99)))}]}}]}]}',
  ...['Between{minimum:-1,maximum:1}','Between{minimum:3,maximum:1}'].map(count=>'Card{name:"Bounds",properties:{card_types:[Instant]},abilities:[Spell{targets:[Target{name:t0,select:Creature,count:'+count+'}],do:[Reveal{objects:Targets(t0)}]}]}'),
  ...['Primitive{name:"Condition::AllCoinsCameUpHeads",operands:[]}','Primitive{name:"Condition::NumCoinFlipsWonIs",operands:[Term{family:"Condition",form:"AllCoinsCameUpHeads",operands:[]}]}'].map(condition=>'Card{name:"Coin participants",properties:{card_types:[Instant]},abilities:[Spell{do:[If{condition:'+condition+',then:[],otherwise:[]}]}]}'),
  ...[
   'drawn=DrawCards{players:You,amount:1},If{condition:CoinFlip{result:drawn,outcome:Heads},then:[],otherwise:[]}',
   'drawn=DrawCards{players:You,amount:1},DrawCards{players:You,amount:CountCoinOutcomes{source:Receipt(drawn),outcome:Heads}}',
   'batch=FlipCoins{players:You,count:1},If{condition:CoinFlip{result:batch,outcome:Heads},then:[],otherwise:[]}'
  ].map(body=>'Card{name:"Coin receipts",properties:{card_types:[Instant]},abilities:[Spell{do:['+body+']}]}')
 ];
 for(const code of malformed)assert.equal((await page.evaluate(code=>window.cardBlocks.parseCode(code),code)).valid,false);
 assert.match((await page.evaluate(code=>window.cardBlocks.parseCode(code),malformed[1])).error,/unbound target slot 99/);
 const invalidArrivals=[
  ['MoveObjects{objects:One(This),destination:OwnersHand,entry:{states:[FaceDown]}}',/battlefield destination/],
  ['MoveTopCard{actor:You,cards:Any,reveal:false,destination:OwnersHand,entry:{tapped:true}}',/battlefield destination/],
  ['Conjure{actor:You,card:Named("Grizzly Bears"),count:1,zone:Hand,spot:Some(Top)}',/library destination/]
 ];
 for(const [operation,error] of invalidArrivals) {
  const result=await page.evaluate(code=>window.cardBlocks.parseCode(code),'Card{name:"Invalid arrival",properties:{card_types:[Instant]},abilities:[Spell{do:['+operation+']}]}');
  assert.equal(result.valid,false);assert.match(result.error,error);
 }
 // A spell can create an independent return-entry event, including its matcher.
 const returnCode='Card{name:"Return event",properties:{card_types:[Instant]},abilities:[Spell{do:[MoveObjects{objects:One(This),destination:Exile,entry:{states:[ExiledForWith{until:EndOfTurn,objects:One(EventObject),do:[NoteObjects{objects:One(EventObject)}]}]}}]}]}';
 const returned=await page.evaluate(code=>window.cardBlocks.parseCode(code),returnCode);
 assert.equal(returned.valid,true,returned.error);
 const returnText=await page.evaluate(text=>window.cardBlocks.formatDocument(text),returned.document);
 assert.equal(returnText.valid,true,returnText.error);assert.equal((returnText.code.match(/EventObject/g)||[]).length,2);
 const badReturn=await page.evaluate(code=>window.cardBlocks.parseCode(code),returnCode.replace('[NoteObjects{objects:One(EventObject)}]','Integer(42)'));
 assert.equal(badReturn.valid,false);assert.match(badReturn.error,/expected a list/);
 assert.deepEqual(errors,[]);
 await page.screenshot({path:process.env.BLOCKLY_SCREENSHOT||'/tmp/card-code.png',fullPage:true});
 console.log(JSON.stringify({codeEditing:true,validation:true,typedTargetReferences:true,typosRejected:true,draftPreserved:true,export:true,search:true,controlSearch:true,sequenceStarter:true,receiptsRetained:true,sharedNaming:true,savedWorkspaceMigration:true,wasmExamples,actionCount,returnEventScope:true,malformedInputsRejected:malformed.length+invalidArrivals.length+1,errors},null,2));
 await browser.close();
})().catch(async e=>{console.error(e);if(page){await page.screenshot({path:'/tmp/card-code-failure.png'});console.error(await page.locator('#code-status').textContent());}if(browser)await browser.close();process.exitCode=1;});
