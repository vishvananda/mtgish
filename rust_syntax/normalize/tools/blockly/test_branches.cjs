/* Actual branch identities, aggregate receipts, copies and persisted v5 names. */
const assert=require('node:assert/strict'),fs=require('node:fs'),{pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
let browser,page;
(async()=>{
 const target=process.argv[2]||'__build/card-blocks.html';
 browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 page=await browser.newPage({viewport:{width:1800,height:1200}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(/^https?:/.test(target)?target:pathToFileURL(require('node:path').resolve(target)).href,{timeout:120000});
 await page.waitForFunction(()=>window.cardBlocks?.valid(),null,{timeout:120000});
 const card=body=>'Card{name:"Branch editor",properties:{card_types:[Instant]},abilities:[Spell{do:['+body+']}]}';
 async function load(source){
  await page.evaluate(async source=>{const d=window.cardBlocks,r=await d.parseCode(source);if(!r.valid)throw Error(r.error);d.loadIR(d.parseExact(r.ir),true,d.parseExact(r.document));},source);
  await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Branch editor');
 }
 await load(card('ChooseOne{decider:You,options:[[picked=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(picked)}],[picked=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(picked)}]],offered:None}'));
 const before=await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated()));
 assert.equal(await page.evaluate(()=>window.cardBlocks.bindings.document().bindings.filter(b=>b.name==='picked').length),2);
 await page.evaluate(()=>{const d=window.cardBlocks,p=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>p.definition&&d.bindings.metadata(p)?.name==='picked');p.block.getField('NAME:'+p.key).showEditor_();});
 await page.locator('input.blocklyHtmlInput').fill('first_picks');await page.locator('input.blocklyHtmlInput').press('Enter');
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.bindings.document().bindings.some(b=>b.name==='first_picks'));
 assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),before);
 const local=await page.evaluate(async()=>{const d=window.cardBlocks,doc=d.bindings.document(),r=await d.formatDocument(d.encode(doc)),port=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>!p.definition&&d.bindings.metadata(p)?.name==='first_picks');return {code:r.code,options:await d.bindings.options(port)};});
 assert.equal((local.code.match(/Selected\(first_picks\)/g)||[]).length,1);assert.equal((local.code.match(/Selected\(picked\)/g)||[]).length,1);assert.equal(local.options.length,1);
 // An exported optional result can be qualified without renaming its outer capture.
 await load(card('picked=BindValue{value:3},Optional{decider:You,do:[picked=MoveObjects{objects:One(This),destination:Exile}]},NoteObjects{objects:Transition{result:ReceiptFrom("optional_1",picked),phase:After}}'));
 const qualified=await page.evaluate(async()=>{const d=window.cardBlocks,doc=d.bindings.document();if(!doc.uses.some(u=>u.qualified))throw Error('Qualified reference metadata was lost');return {workspace:d.savedWorkspace(),ir:d.encode(d.generated()),code:(await d.formatDocument(d.encode(doc))).code};});
 assert.match(qualified.code,/ReceiptFrom\("optional_1", picked\)/);
 await page.evaluate(saved=>window.cardBlocks.restore(saved),qualified.workspace);await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.bindings.document().uses.some(u=>u.qualified));
 assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),qualified.ir);
 const privateChoice='ChooseOne{decider:You,options:[[r11=MoveObjects{objects:One(This),destination:Exile}],[r11=MoveObjects{objects:One(This),destination:Exile}]],offered:None}';
 await load(card(privateChoice+',ForEachPlayer{players:All,binding:each,in_chosen_direction:false,do:['+privateChoice+']},JoinResults{results:[r11,r11]}'));
 assert.equal(await page.evaluate(()=>window.cardBlocks.bindings.document().bindings.filter(b=>b.members?.length).length),1);
 // A shared numeric receipt retains separate declarations and an aggregate read.
 await load(card('Group([ChooseOne{decider:You,options:[[r11=MoveObjects{objects:One(This),destination:Exile}],[r11=MoveObjects{objects:One(This),destination:Exile}]],offered:None},joined=JoinResults{results:[r11,r11]},NoteObjects{objects:Transition{result:joined,phase:After}}])'));
 const shared=await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated()));
 for(const [index,name] of [[0,'left_receipt'],[1,'right_receipt']]){
  await page.evaluate(async({index,name})=>{const d=window.cardBlocks,doc=d.bindings.document(),group=doc.bindings.find(b=>b.members?.length),binding=group.members[index],port=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>p.definition&&d.bindings.metadata(p)?.name===doc.bindings[binding].name&&d.bindings.metadata(p)?.scope_name===doc.bindings[binding].scope_name);await d.bindings.rename(d.bindings.metadata(port).key,name);},{index,name});
  await page.waitForFunction(name=>window.cardBlocks.valid()&&window.cardBlocks.bindings.document().bindings.some(b=>b.name===name),name);
 }
 assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),shared);
 const aggregate=await page.evaluate(async()=>{const d=window.cardBlocks,doc=d.bindings.document(),port=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>!p.definition&&d.bindings.metadata(p)?.members?.length);return {code:(await d.formatDocument(d.encode(doc))).code,label:port.block.getField('REF:'+port.key).getText()};});
 assert.match(aggregate.label,/left_receipt/);assert.match(aggregate.label,/right_receipt/);assert.equal((aggregate.code.match(/left_receipt/g)||[]).length,3);assert.equal((aggregate.code.match(/right_receipt/g)||[]).length,3);
 const clip=await page.evaluate(()=>{const d=window.cardBlocks,ability=d.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='Ability');return Blockly.serialization.blocks.save(ability.getInputTargetBlock('F_body'),{addNextBlocks:true});});
 for(const foreign of [false,true]) {
  await page.evaluate(({clip,foreign})=>{const d=window.cardBlocks;
   function visit(node){for(const m of Object.values(node.extraState?.bindings||{}))m.key='foreign:'+m.key;for(const input of Object.values(node.inputs||{})){if(input.block)visit(input.block);if(input.shadow)visit(input.shadow);}if(node.next?.block)visit(node.next.block);}
   if(foreign)visit(clip);const copy=Blockly.serialization.blocks.append(clip,d.workspace),ability=d.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='Ability');let tail=ability.getInputTargetBlock('F_body');while(tail.getNextBlock())tail=tail.getNextBlock();tail.nextConnection.connect(copy.previousConnection);
  },{clip,foreign});
  await page.waitForFunction(count=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions.length===count,foreign?3:2);
 }
 const copies=await page.evaluate(()=>{const d=window.cardBlocks,doc=d.bindings.document();return {groups:doc.bindings.filter(b=>b.members?.length).map(b=>b.members.map(i=>doc.bindings[i].name)),ir:d.generated()};});
 assert.equal(copies.groups.length,3);assert.ok(copies.groups.every(g=>g.length===2));
 const copiedIds=copies.ir.faces[0].abilities[0].args.body.instructions.map(g=>g.args.instructions[0].args.options.map(p=>p.instructions[0].args.result));
 assert.ok(copiedIds.every(pair=>pair[0]===pair[1]));assert.equal(new Set(copiedIds.map(pair=>pair[0])).size,3);
 // Upgrade an actual v5 save whose alternatives shared authoring identities.
 const old=require('./fixtures/workspace-v5-branches.json');
 const original=await page.evaluate(old=>window.cardBlocks.encode(window.cardBlocks.fromBlock(window.cardBlocks.fakeBlock(old.workspace.blocks.blocks[0]))),old);
 await page.evaluate(old=>window.cardBlocks.restore(old),old);
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Legacy branch names'&&window.cardBlocks.bindings.document().bindings.filter(b=>b.name==='picked').length===4);
 assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),original);
 assert.equal(await page.evaluate(()=>window.cardBlocks.savedWorkspace().presentation),6);
 assert.deepEqual(await page.evaluate(()=>JSON.parse(localStorage.getItem('mtgish.blockly.before-authoring-v6'))),old);
 await page.evaluate(async()=>{const d=window.cardBlocks,p=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>p.definition&&d.bindings.metadata(p)?.name==='picked');await d.bindings.rename(d.bindings.metadata(p).key,'migrated_branch');});
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.bindings.document().bindings.some(b=>b.name==='migrated_branch'));
 await page.reload();await page.waitForFunction(()=>window.cardBlocks?.valid()&&window.cardBlocks.bindings.document().bindings.some(b=>b.name==='migrated_branch'),null,{timeout:120000});
 assert.equal(await page.evaluate(()=>window.cardBlocks.bindings.document().bindings.filter(b=>b.name==='picked').length),3);
 // Both authoring-document versions import through the native migration result.
 const legacy=fs.readFileSync(require('node:path').resolve(__dirname,'../../tests/fixtures/authoring-v1-if.json'));
 await page.locator('#file').setInputFiles({name:'legacy-document.json',mimeType:'application/json',buffer:legacy});
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Branch names'&&window.cardBlocks.bindings.document().bindings.filter(b=>b.name==='picked').length===2);
 const document=await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.bindings.document()));
 await page.locator('#file').setInputFiles({name:'current-document.json',mimeType:'application/json',buffer:Buffer.from(document)});
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.bindings.document().version===2);
 assert.deepEqual(errors,[]);
 console.log(JSON.stringify({independentBranches:true,localOptions:true,qualifiedExports:true,privateLoopGroups:true,aggregateReceipts:true,irIdsRetained:true,localAndForeignCopies:true,v5Migration:true,reload:true,documentImports:true,errors},null,2));
 await browser.close();
})().catch(async e=>{console.error(e);if(browser)await browser.close();process.exitCode=1});
