/* Literal shadows and expression blocks occupy the same typed sockets. */
const assert=require('node:assert/strict');
const {pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
let browser,page;
(async()=>{
 const input=process.argv[2]||'__build/card-blocks.html';
 browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 page=await browser.newPage({viewport:{width:1580,height:1000}});page.setDefaultTimeout(30000);
 const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(/^https?:/.test(input)?input:pathToFileURL(require('node:path').resolve(input)).href,{timeout:120000});
 await page.waitForFunction(()=>window.cardBlocks?.valid());
 await page.locator('#starter').selectOption('draw');await page.locator('#new-card').click();
 await page.waitForFunction(()=>window.cardBlocks.valid()&&document.getElementById('card-title').textContent==='New draw spell');
 const starter=await page.evaluate(()=>{
  const d=window.cardBlocks,b=d.workspace.getAllBlocks(false).find(b=>b.state.op==='Draw');
  return {amount:b.getInputTargetBlock('O_amount').isShadow(),players:b.getInputTargetBlock('O_players').isShadow(),
   amountType:b.getInput('O_amount').connection.getCheck(),playersType:b.getInput('O_players').connection.getCheck(),
   menu:b.getField('MORE').getOptions(false).map(([label])=>label)};
 });
 assert.equal(starter.amount,true);assert.equal(starter.players,true);
 assert.deepEqual(starter.amountType.map(JSON.parse),[{named:'Expr'}]);assert.deepEqual(starter.playersType.map(JSON.parse),[{named:'PlayerSet'}]);
 assert.ok(starter.menu.every(s=>!s.startsWith('Use a block')));
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op==='Draw').getInputTargetBlock('O_amount').setFieldValue('3','_VALUE'));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount.args===3);
 await page.evaluate(()=>window.cardBlocks.workspace.undo(false));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount.args===1);
 await page.evaluate(()=>window.cardBlocks.workspace.undo(true));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount.args===3);
 // Drag an actual toolbox expression onto the existing editable number.
 await page.locator('.blocklyToolboxCategory').filter({has:page.getByText('Values',{exact:true})}).first().click();
 await page.evaluate(async()=>{window.cardBlocks.applyPresentation();await Blockly.renderManagement.finishQueuedRenders();});
 const start=await page.evaluate(()=>{
  const b=window.cardBlocks.workspace.getFlyout().getWorkspace().getAllBlocks(false).find(b=>b.state.ty.named==='Expr'&&b.state.variant==='AnnouncedX');
  const r=b.getSvgRoot().getBoundingClientRect();return {x:r.x+18,y:r.y+15};
 });
 await page.mouse.move(start.x,start.y);await page.mouse.down();await page.mouse.move(1050,300,{steps:25});await page.waitForTimeout(150);
 const offset=await page.evaluate(()=>{
  const w=window.cardBlocks.workspace,b=w.getTopBlocks(false).find(b=>b.state.ty.named==='Expr'&&b.state.variant==='AnnouncedX');
  const socket=w.getAllBlocks(false).find(b=>b.state.op==='Draw').getInput('O_amount').connection;
  const target=Blockly.utils.svgMath.wsToScreenCoordinates(w,new Blockly.utils.Coordinate(socket.x,socket.y));
  const r=b.getSvgRoot().getBoundingClientRect(),off=b.outputConnection.getOffsetInBlock();
  return {dx:target.x-(r.x+off.x*w.scale),dy:target.y-(r.y+off.y*w.scale)};
 });
 await page.mouse.move(1050+offset.dx,300+offset.dy,{steps:25});await page.waitForTimeout(100);await page.mouse.up();
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount.kind==='AnnouncedX');
 await page.evaluate(()=>{
  const d=window.cardBlocks,b=d.workspace.getAllBlocks(false).find(b=>b.state.op==='Draw');
  if(b.getInputTargetBlock('O_amount').isShadow())throw Error('Expression stayed a shadow');
  const saved=d.savedWorkspace(),before=d.encode(d.generated());d.restore(saved);
  if(d.encode(d.generated())!==before)throw Error('Saving changed expression IR');
 });
 await page.waitForFunction(()=>window.cardBlocks.valid());
 // Removing an expression restores the previous literal, including its edited value.
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op==='Draw').getInputTargetBlock('O_amount').dispose(false));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount.args===3);
 await page.evaluate(()=>window.cardBlocks.workspace.undo(false));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount.kind==='AnnouncedX');
 await page.evaluate(()=>window.cardBlocks.workspace.undo(true));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount.args===3);
 // Rebuilding an action retains its active shadow values.
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op==='Draw').setFieldValue('GainLife','VARIANT'));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.kind==='GainLife');
 assert.equal(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount.args),3);
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op==='GainLife').setFieldValue('Draw','VARIANT'));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.kind==='Draw');
 // Compound player selection uses the same socket as the simple picker.
 await page.evaluate(()=>{
  const d=window.cardBlocks,w=d.workspace,b=w.getAllBlocks(false).find(b=>b.state.op==='Draw');
  const selection={kind:'Union',args:[{kind:'One',args:{kind:'AbilityController'}},{kind:'OpponentsOf',args:{kind:'AbilityController'}}]};
  const block=Blockly.serialization.blocks.append(d.blockState({named:'PlayerSet'},selection),w);
  b.getInput('O_players').connection.connect(block.outputConnection);
 });
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.players.kind==='Union');
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op==='Draw').getInputTargetBlock('O_players').dispose(false));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.players.kind==='One');
 const old=require('./fixtures/workspace-v3.json');
 const migrated=await page.evaluate(async old=>{
  const d=window.cardBlocks;if(old.workspace.schema===d.schema.version)d.restore(old.workspace);else await d.migrateWorkspace(old.workspace);
  return {ir:d.generated(),presentation:d.savedWorkspace().presentation,
   shadow:d.workspace.getAllBlocks(false).find(b=>b.state.op==='Draw').getInputTargetBlock('O_amount').isShadow()};
 },old);
 const upgraded=await page.evaluate(ir=>window.cardBlocks.migrateIR(window.cardBlocks.encode(ir)),old.ir);
 assert.equal(upgraded.valid,true);assert.deepEqual(migrated.ir,JSON.parse(upgraded.ir));assert.equal(migrated.presentation,6);assert.equal(migrated.shadow,true);
 await page.waitForFunction(()=>window.cardBlocks.valid());
 assert.deepEqual(errors,[]);
 await page.screenshot({path:process.env.BLOCKLY_SCREENSHOT||'/tmp/typed-sockets.png',fullPage:true});
 console.log(JSON.stringify({literalEditing:true,realToolboxDrag:true,typedSockets:true,restoreLiteral:true,playerExpressions:true,undoRedo:true,persistence:true,legacyMigration:true,rebuildValues:true,errors},null,2));
 await browser.close();
})().catch(async e=>{console.error(e);if(page)await page.screenshot({path:'/tmp/typed-sockets-failure.png'});if(browser)await browser.close();process.exitCode=1;});
