/* Destination-specific movement and genuine schema-185 named workspace upgrades. */
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),{pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');let browser;
(async()=>{
 const input=process.argv[2]||'__build/card-blocks.html';browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 const page=await browser.newPage({viewport:{width:1580,height:1000}}),errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(/^https?:/.test(input)?input:pathToFileURL(path.resolve(input)).href,{timeout:120000});await page.waitForFunction(()=>window.cardBlocks?.valid(),null,{timeout:120000});
 const epoch=await page.evaluate(()=>window.cardBlocks.schema.version);assert.ok(epoch>=186);
 assert.deepEqual(await page.evaluate(()=>{const t=window.cardBlocks.schema.types;return [!!t.MovementDestination,!!t.BattlefieldArrival,!!t.ExileArrival,!!t.CardArrival,!!t.ConjureDestination];}),[true,true,true,true,true]);
 assert.deepEqual(await page.evaluate(()=>window.cardBlocks.schema.types.Operation.variants.find(v=>v.wire==='Move').fields.fields.map(f=>f.name)),['objects','destination','qualifiers']);
 // Edit actual destination blocks; modifiers follow the chosen zone.
 const editCode='Card{name:"Destination edit",properties:{card_types:[Instant]},abilities:[Spell{do:[MoveObjects{objects:One(This),destination:Exile{}}]}]}';
 await page.evaluate(async code=>{const d=window.cardBlocks,r=await d.parseCode(code);if(!r.valid)throw Error(r.error);d.loadIR(d.parseExact(r.ir),false,d.parseExact(r.document));},editCode);
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Destination edit');
 await page.locator('#details').click();
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='MovementDestination').setFieldValue('FaceDown','_F_visibility'));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.destination.Exile.visibility==='FaceDown');
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='MovementDestination').setFieldValue('Library','VARIANT'));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&!!window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.destination.Library);
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='LibraryPosition').setFieldValue('Bottom','VARIANT'));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.destination.Library.position==='Bottom');
 await page.locator('#details').click();
 assert.equal(await page.evaluate(()=>{const b=window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='MovementDestination');return b.getInput('F_arrival').isVisible();}),false);
 await page.locator('#blocks-tab').click();
 await page.screenshot({path:process.env.BLOCKLY_SCREENSHOT||'/tmp/movement-destinations.png',fullPage:true});
 // A meaningful arrival from an incompatible zone remains recoverable.
 await page.evaluate(()=>{const d=window.cardBlocks,ir=d.generated();ir.faces[0].abilities[0].args.body.instructions[0].args.operation.args.destination={Exile:{visibility:'FaceUp',arrival:{counters:[],states:[{ExiledFor:'EndOfTurn'}]}}};d.loadIR(ir,false);});
 await page.waitForFunction(()=>window.cardBlocks.valid());
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='MovementDestination').setFieldValue('Library','VARIANT'));
 await page.waitForFunction(()=>window.cardBlocks.workspace.getTopBlocks(false).some(b=>b.state.ty.named==='ExileArrival'));
 assert.equal(await page.evaluate(()=>{const d=window.cardBlocks,b=d.workspace.getTopBlocks(false).find(b=>b.state.ty.named==='ExileArrival');return d.fromBlock(b).states[0].ExiledFor;}),'EndOfTurn');
 assert.equal(await page.evaluate(()=>!!window.cardBlocks.valid()),false);
 await page.evaluate(()=>window.cardBlocks.workspace.undo(false));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&!!window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.destination.Exile);
 function keys(node,out=new Set()){for(const m of Object.values(node.extraState?.bindings||{}))out.add(m.key);for(const v of Object.values(node.inputs||{})){if(v.block)keys(v.block,out);if(v.shadow)keys(v.shadow,out);}if(node.next?.block)keys(node.next.block,out);return [...out].sort();}
 const cases=require('./fixtures/workspaces-movement-v6-schema185.json');assert.equal(cases.length,6);
 for(const {name,document,workspace:old} of cases) {
  const expected=await page.evaluate(async doc=>{const d=window.cardBlocks,r=await d.formatDocument(d.encode(doc));if(!r.valid)throw Error(r.error);return d.parseExact(r.document);},document),oldKeys=keys(old.workspace.blocks.blocks[0]);
  async function checked(){await page.waitForFunction(expected=>{const d=window.cardBlocks;if(!d?.valid())return false;const c=d.generated();return c.layout===expected.card.layout&&JSON.stringify(c.faces.map(f=>f.name))===JSON.stringify(expected.card.faces.map(f=>f.name));},expected);
   const actual=await page.evaluate(()=>{const d=window.cardBlocks;return {ir:d.encode(d.generated()),doc:d.bindings.document(),save:d.savedWorkspace(),backup:JSON.parse(localStorage.getItem('mtgish.blockly.before-schema-upgrade'))};});
   assert.deepEqual(JSON.parse(actual.ir),expected.card,name);assert.deepEqual(actual.doc.bindings,expected.bindings,name);assert.deepEqual(keys(actual.save.workspace.blocks.blocks[0]),oldKeys,name);assert.equal(actual.save.schema,epoch);assert.deepEqual(actual.backup,old);
  }
  await page.locator('#file').setInputFiles({name:'old-named.workspace.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(old))});await checked();
  await page.evaluate(old=>localStorage.setItem('mtgish.blockly.v1',JSON.stringify(old)),old);await page.reload();await page.waitForFunction(()=>window.cardBlocks?.ready(),null,{timeout:120000});await checked();
 }
 const before=await page.evaluate(()=>JSON.stringify(window.cardBlocks.savedWorkspace())),bad=structuredClone(cases[0].workspace);bad.schema=0;
 const error=await page.evaluate(async old=>{try{await window.cardBlocks.migrateWorkspace(old);return null;}catch(e){return e.message;}},bad);assert.match(error,/Unsupported saved-workspace schema/);assert.equal(await page.evaluate(()=>JSON.stringify(window.cardBlocks.savedWorkspace())),before);
 assert.deepEqual(errors,[]);console.log(JSON.stringify({schema:epoch,typedMovementDestinations:true,actualDestinationEdits:true,emptyArrivalHidden:true,nonemptyArrivalPreserved:true,destinationUndo:true,named185Import:6,named185Startup:6,bindingKeysPreserved:true,failedUpgradePreservesDraft:true,errors},null,2));await browser.close();
})().catch(async e=>{console.error(e);if(browser)await browser.close();process.exitCode=1;});
