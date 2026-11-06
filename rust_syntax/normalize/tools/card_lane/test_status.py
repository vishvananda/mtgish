import importlib.util
import json
import os
import pty
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

spec = importlib.util.spec_from_file_location('card_status', Path(__file__).with_name('status.py'))
status = importlib.util.module_from_spec(spec)
spec.loader.exec_module(status)

class StatusTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='card-status-test-')
        self.root = Path(self.tmp.name)
        self.state = self.root / 'new-project' / 'worker-a' / 'state'
        self.state.mkdir(parents=True)
    def tearDown(self):
        self.tmp.cleanup()
    def test_dashboard_discovers_named_campaign_and_direct_campaign_root(self):
        folder=self.root/'my-cards';folder.mkdir()
        (folder/'config.json').write_text(json.dumps({'reviewers':{}}))
        (folder/'status.json').write_text(json.dumps({'counts':[]}))
        self.assertEqual(len(status.pipelines(self.root,[])),1)
        self.assertEqual(len(status.pipelines(folder,[])),1)
    def test_command_uses_configured_discovery_home(self):
        result=subprocess.run([sys.executable,str(Path(status.__file__)),'--json'],
                              env={**os.environ,'CARD_LANE_HOME':str(self.root)},
                              text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(result.stdout)['root'],str(self.root.resolve()))
    def test_live_cpu_averages_and_regression_progress(self):
        folder=self.root/'pipeline-resources';folder.mkdir()
        packet=folder/'jobs/a/review';packet.mkdir(parents=True)
        worker=folder/'workers/r1';worker.mkdir(parents=True)
        (worker/'current.json').write_text(json.dumps({'name':'A','folder':str(packet)}))
        (folder/'status.json').write_text(json.dumps({'counts':[], 'active':[{'name':'A','worker':'r1','stage':'review'}]}))
        (packet/'checks-live.json').write_text(json.dumps({'card':'A','at':time.time(),'state':'running',
             'phase':'caller regressions','gate':'B coverage','card_index':40,'total_cards':123}))
        resource=folder/'resources';resource.mkdir()
        (resource/'status.json').write_text(json.dumps({'at':time.time(),'sampled_seconds':300,'logical_cpus':88,
             'host_cpu_cores':{'mean':30},'roles':{name:{'cpu_cores':{'mean':v}} for name,v in [('implement',6),('review',4),('verify',.1)]}}))
        pipes=status.pipelines(self.root,[])
        text=status.render([],self.root,width=120,pipeline=pipes)
        self.assertIn('CPU avg (5.0m)  IMPL 6.0 · REVIEW 4.0 · VERIFY 0.1 · HOST 30.0/88 logical CPUs',text)
        self.assertIn('REGRESSION',text)
        self.assertIn('40/123 B coverage',text)
    def test_rolling_phase_rates_survive_narrow_terminals_and_json_loading(self):
        folder=self.root/'pipeline-rates';folder.mkdir()
        rates={'window_seconds':1800, 'phases':{name:{'advanced_per_hour':rate} for name,rate in
               [('implementation',72),('review',42),('fix',6),('verify',38)]}}
        (folder/'status.json').write_text(json.dumps({'counts':[], 'phase_rates':rates}))
        pipes=status.pipelines(self.root,[])
        self.assertEqual(pipes[0]['phase_rates'],rates)
        for width in (60,120):
            text=status.render([],self.root,width=width,pipeline=pipes)
            self.assertIn('PHASE RATES (30m)',text)
            self.assertIn('IMPL 1.2/min',text)
            self.assertIn('REVIEW 42.0/h',text)
            self.assertIn('FIX 6.0/h',text)
            self.assertIn('VERIFY 38.0/h',text)
            self.assertTrue(all(len(line)==width for line in text.splitlines()))
    def test_pipeline_cost_average_is_weighted_and_card_costs_are_visible(self):
        for i, count, completed in [(1, 2, 1), (2, 3, 9)]:
            folder=self.root/f'pipeline-cost-{i}';folder.mkdir()
            entry={'name':'A','status':'passed','usd_api_equivalent':.123,'stages':{'implement':.02,'review':.1,'verify':.003}}
            (folder/'status.json').write_text(json.dumps({'counts':[], 'costs':{
                'completed_cards':count,'completed_usd':completed,'total_usd':completed+2,'cards':{'A':entry}},
                'recent':[{'worker':'r1','kind':'passed','name':'A','detail':{},'at':'2026-11-03T16:00:00+00:00'}]}))
        pipes=status.pipelines(self.root,[])
        text=status.render([],self.root,width=160,pipeline=pipes)
        self.assertIn('AVG $2.000/card · 5 completed · TOTAL $14.00 · API est',text)
        self.assertIn('A · $0.123 · review passed',text)
        table=status.render_costs(pipes)
        self.assertIn('IMPLEMENT',table)
        self.assertIn('0.1230',table)
        self.assertIn('0.0030',table)
    def queue(self, cards):
        (self.state / 'queue.json').write_text(json.dumps({'format': 'example', 'cards': cards}))
    def test_passed_is_not_committed_and_closed_work_is_counted_once(self):
        self.queue([{'name': 'A', 'status': 'committed'}, {'name': 'B', 'status': 'working'}, {'name': 'C', 'status': 'working'}, {'name': 'D', 'status': 'pending'}])
        batch = self.state / 'batch'; batch.mkdir()
        (batch / 'stream.json').write_text(json.dumps({'current': 'D', 'closed': {'A': {'outcome': 'passed'}, 'B': {'outcome': 'passed'}, 'C': {'outcome': 'engine_gap'}}}))
        run = status.collect(self.root, {str(self.state): {'driver': [123], 'model': []}})[0]
        self.assertEqual((run['committed'], run['passed_waiting'], run['blocked'], run['done'], run['total']), (1, 1, 1, 3, 4))
        self.assertEqual(run['phase'], 'checking')
    def test_abandoned_working_queue_is_stopped(self):
        self.queue([{'name': 'A', 'status': 'working'}])
        self.assertEqual(status.collect(self.root, {})[0]['phase'], 'stopped')
    def test_caches_and_packets_are_not_runs(self):
        self.queue([])
        for p in [self.root/'target'/'state', self.state/'cards'/'A']:
            p.mkdir(parents=True); (p/'queue.json').write_text('{"cards": []}')
        self.assertEqual(len(status.collect(self.root, {})), 1)
    def test_recovery_archives_are_not_worker_runs(self):
        self.queue([])
        backup=self.state.parent/'capacity-recovery-20260101';backup.mkdir()
        (backup/'queue.json').write_text('{"cards": []}')
        self.assertEqual(len(status.collect(self.root,{})),1)
    def test_half_written_json_is_safe(self):
        (self.state/'queue.json').write_text('{"cards":')
        self.assertEqual(status.collect(self.root, {}), [])
    def test_complete_queue_does_not_count_old_stream_passes_again(self):
        self.queue([{'name': 'A', 'status': 'committed'}])
        self.assertEqual(status.collect(self.root, {})[0]['phase'], 'complete')
    def test_names_file_limits_large_generator_backlog(self):
        cards = [{'name': 'Assigned A', 'status': 'committed'}, {'name': 'Assigned B', 'status': 'engine_gap'}]
        cards += [{'name': f'Backlog {n}', 'status': 'pending'} for n in range(1000)]
        self.queue(cards)
        (self.state.parent/'names.txt').write_text('Assigned A\nAssigned B\n')
        batch=self.state/'batch';batch.mkdir()
        (batch/'stream.json').write_text(json.dumps({'closed': {'Unrelated old card': {'outcome':'passed'}}}))
        run=status.collect(self.root,{})[0]
        self.assertEqual((run['total'],run['done'],run['committed'],run['blocked']), (2,2,1,1))
        self.assertEqual(run['backlog_total'],1002)
        self.assertEqual(run['assignment_source'],'names.txt')
    def test_color_layout_preserves_columns_and_terminal_height(self):
        self.queue([{'name':'A','status':'working'}])
        runs=status.collect(self.root,{str(self.state):{'driver':[1],'model':[2]}})
        text=status.render(runs,self.root,1,8,96,24,True)
        self.assertIn('\033[',text)
        self.assertLessEqual(len(text.splitlines()),24)
        for line in text.splitlines():self.assertEqual(len(status.ANSI.sub('',line)),96)
    def test_existing_failure_keeps_its_time_when_stream_advances(self):
        self.queue([{'name':n,'status':'working'} for n in ('A','B','C','D')])
        batch=self.state/'batch';batch.mkdir();path=batch/'stream.json'
        stream={'opened':1000,'current':'C','closed':{'A':{'outcome':'needs_review','seconds':10},'B':{'outcome':'passed','seconds':20}}}
        path.write_text(json.dumps(stream))
        first=status.collect(self.root,{})[0]
        times={e['name']:e['time'] for e in first['events']}
        self.assertEqual(times,{'A':980,'B':1000})
        stream['opened']=1030;stream['current']='D';stream['closed']['C']={'outcome':'passed','seconds':30};path.write_text(json.dumps(stream))
        second=status.collect(self.root,{})[0];later={e['name']:e['time'] for e in second['events']}
        self.assertEqual(later,{'A':980,'B':1000,'C':1030})
    def test_worker_order_does_not_follow_file_modification_time(self):
        live={}
        for name in ('w10','w2','w1'):
            state=self.root/'campaign'/name/'state';state.mkdir(parents=True)
            (state/'queue.json').write_text(json.dumps({'built':'2026-01-01T00:00:00+00:00','cards':[{'name':name,'status':'working'}]}))
            live[str(state)]={'driver':[1],'model':[]}
        before=[r['run'] for r in status.collect(self.root,live)]
        os.utime(self.root/'campaign/w10/state/queue.json',(2000000000,2000000000))
        after=[r['run'] for r in status.collect(self.root,live)]
        self.assertEqual(before,after)
        self.assertEqual(after,['campaign/w1','campaign/w2','campaign/w10'])
    def test_rates_exclude_blockers_and_use_parallel_wall_time(self):
        runs=[{'committed':2,'passed_waiting':3,'blocked':50,'rate_start':100,'rate_end':400}, {'committed':1,'passed_waiting':4,'blocked':10,'rate_start':200,'rate_end':400}]
        value=status.rate_stats(runs)
        self.assertEqual(value['validated'],10)
        self.assertEqual(value['elapsed_seconds'],300)
        self.assertEqual(value['per_hour'],120)
        self.assertEqual(status.format_rate(120),'2.0 cards/min')
        self.assertEqual(status.format_rate(12),'12.0 cards/hour')
    def test_historical_rate_does_not_decay(self):
        self.queue([{'name':'A','status':'committed','started':'2026-01-01T00:00:00+00:00','finished':'2026-01-01T00:05:00+00:00'}])
        run=status.collect(self.root,{})[0]
        self.assertEqual(run['rate_per_hour'],12)
    def test_dispatcher_discovery_and_stale_heartbeat(self):
        controller=self.root/'campaign';controller.mkdir()
        (controller/'dispatcher-state.json').write_text(json.dumps({'checked_at':1,'enabled':True,'active':5,'max_workers':5,'batch_size':20,'eligible_waiting':40,'generator_gaps':2}))
        self.assertEqual(status.dispatchers(self.root,'other'),[])
        controllers=status.dispatchers(self.root,'campaign')
        text=status.render([],self.root,width=140,dispatch=controllers)
        self.assertIn('AUTO STALE',text)
        self.assertIn('5/5 slots',text)
        self.assertIn('40 waiting',text)
    def test_pipeline_counts_commits_separately_and_uses_matching_live_worker(self):
        self.queue([{'name':'Implemented','status':'committed'}, {'name':'Pending','status':'pending'}])
        folder=self.root/'pipeline-example';folder.mkdir()
        (folder/'config.json').write_text(json.dumps({'lane_root':str(self.root/'new-project'),'cards_per_context':20}))
        (folder/'status.json').write_text(json.dumps({'counts':[{'stage':'review','status':'queued','cards':136},{'stage':'review','status':'running','cards':1}], 'active':[{'worker':'r1','name':'Review card','stage':'review'}], 'recent':[{'worker':'r1','detail':{'context':{'last_tokens':42000}}}]}))
        worker=folder/'workers/r1';worker.mkdir(parents=True)
        (worker/'live.json').write_text(json.dumps({'state':'running','card':'Review card','context_cards':3,'model':'terra'}))
        runs=status.collect(self.root,{})
        pipes=status.pipelines(self.root,runs)
        self.assertEqual(pipes[0]['implementation_committed'],1)
        text=status.render(runs,self.root,width=120,pipeline=pipes)
        self.assertIn('impl committed 1',text)
        self.assertIn('review queued 136, running 1',text)
        self.assertIn('Review card',text)
        self.assertIn('ROLE       PHASE',text)
        self.assertNotIn('ctx card',text)
        self.assertEqual(pipes[0]['workers'][0]['latest_context_tokens'],42000)
        self.assertEqual(status.pipelines(self.root,runs,'unrelated'),[])
        (worker/'live.json').write_text(json.dumps({'state':'running','card':'Old card','context_cards':19,'model':'old'}))
        self.assertIsNone(status.pipelines(self.root,runs)[0]['workers'][0]['context_cards'])
    def test_pipeline_active_filter_and_missing_optional_data(self):
        folder=self.root/'pipeline-example';folder.mkdir()
        (folder/'status.json').write_text(json.dumps({'counts':[{'stage':'review','status':'passed','cards':2}], 'active':[]}))
        pipes=status.pipelines(self.root,[])
        self.assertIsNone(pipes[0]['implementation_committed'])
        self.assertIn('review passed 2',status.render([],self.root,width=60,pipeline=pipes))
        self.assertEqual(status.pipelines(self.root,[],active_only=True),[])

    def test_local_consolidation_replaces_the_pr_stage(self):
        folder=self.root/'pipeline-local';folder.mkdir()
        (folder/'config.json').write_text(json.dumps({'workflow':'consolidate_then_publish','integration':{'branch':'integration/local'}}))
        (folder/'status.json').write_text(json.dumps({'counts':[],'active':[], 'flow':{'ready':{'cards':20},'blocked':{'cards':2}}}))
        (folder/'integration').mkdir()
        (folder/'integration/status.json').write_text(json.dumps({'counts':{'assembled':42,'pending':7,'upstream':3},
            'verified':{'collected':40,'to_collect':9,'upstream':3,'confirmed':52}}))
        pipes=status.pipelines(self.root,[]);text=status.render([],self.root,width=150,pipeline=pipes)
        self.assertIn('40 VERIFIED LOCALLY',text)
        self.assertIn('│LOCAL',text)
        self.assertIn('verified 40',text)
        self.assertIn('to collect 9',text)
        self.assertNotIn('assembled 42',text)
        self.assertIn('publish at end',text)
        self.assertNotIn('PR READY',text)
        self.assertNotIn('│PR',text)

    def test_idle_verifier_keeps_its_row_and_order(self):
        folder=self.root/'pipeline-example';folder.mkdir()
        (folder/'config.json').write_text(json.dumps({'enabled':True, 'reviewers':{'r5':{'role':'confirm'},'r1':{'role':'review'}},'review_model':'terra','cards_per_context':20}))
        (folder/'status.json').write_text(json.dumps({'counts':[], 'active':[{'worker':'r1','name':'Work','stage':'review'}]}))
        worker=folder/'workers/r5';worker.mkdir(parents=True)
        (worker/'live.json').write_text(json.dumps({'state':'waiting','at':time.time(),'context_cards':2}))
        pipes=status.pipelines(self.root,[])
        self.assertEqual([w['worker'] for w in pipes[0]['workers']],['r1','r5'])
        self.assertEqual(pipes[0]['workers'][1]['state'],'ready')
        text=status.render([],self.root,width=150,pipeline=pipes)
        self.assertRegex(text,r'r5\s+verify\s+READY')
        self.assertIn('waiting for confirmation',text)
        (worker/'live.json').write_text(json.dumps({'state':'waiting','at':time.time()-60}))
        self.assertEqual(status.pipelines(self.root,[])[0]['workers'][1]['state'],'stale')
    def test_flow_separates_review_fix_rounds_and_shared_workers(self):
        folder=self.root/'pipeline-flow';folder.mkdir()
        def node(cards, workers=0, active=0, rounds=None):
            return {'cards':cards,'workers':workers,'active_workers':active,'rounds':rounds or [0,0,0,0]}
        flow={'intake':dict(node(7), queued=7, held=3), 'implementation':node(0,5,0),'review':node(12,4,2,[12,0,0,0]),
              'fix':node(6,4,1,[0,3,2,1]),'verify':node(3,1,1,[0,1,1,1]),
              'ready':node(18),'publishing':node(2),'published':node(20),'blocked':node(2,rounds=[0,1,0,1])}
        (folder/'status.json').write_text(json.dumps({'counts':[], 'active':[], 'flow':flow, 'recent':[{'worker':'r1','kind':'passed','name':'A','at':'2026-01-01T00:00:00Z','detail':{}}]}))
        pipes=status.pipelines(self.root,[])
        self.queue([{'name':'Old','status':'committed'}]);runs=status.collect(self.root,{})
        text=status.render(runs,self.root,width=120,height=40,pipeline=pipes)
        self.assertIn('│REVIEW / FIX',text)
        self.assertIn('│3/4 active (shared)',text)
        self.assertIn('│Review 12 cards',text)
        self.assertIn('│Fix 6 cards',text)
        self.assertIn('R0:12 R1:0 R2:0 R3:0',text)
        self.assertIn('R0:0 R1:3 R2:2 R3:1',text)
        self.assertIn('│R0:0 R1:1',text)
        self.assertIn('│R2:1 R3:1',text)
        self.assertIn('│gather 18/20',text)
        self.assertIn('│INTAKE',text)
        self.assertIn('│Queued 7',text)
        self.assertIn('│Held 3',text)
        self.assertLess(text.index('│INTAKE'),text.index('│IMPLEMENT'))
        self.assertTrue(all(len(line)==120 for line in text.splitlines()))
        self.assertIn('│BLOCKED',text)
        right = next(line for line in text.splitlines() if 'Review 12 cards' in line)
        left = next(line for line in text.splitlines() if ' ← ' in line)
        self.assertEqual(right.index(' → ', right.index('Review')),left.index(' ← '))
        headers = next(line for line in text.splitlines() if '│REVIEW / FIX' in line)
        self.assertLess(headers.index('│PR'),headers.index('│BLOCKED'))
        self.assertNotIn('├ PIPELINE ',text)
        self.assertNotIn('VERIFY rejection',text)
        colored=status.render(runs,self.root,width=120,color=True,pipeline=pipes)
        self.assertIn('\033[1;31m│BLOCKED',colored)
        self.assertIn('│ → │',text)
        self.assertIn('RECENT OUTCOMES',text)
        self.assertNotIn('RECENT RUNS',text)
        self.assertLessEqual(len(text.splitlines()),40)
        narrow=status.render(runs,self.root,width=60,height=40,pipeline=pipes)
        self.assertIn('│REVIEW / FIX',narrow)
        self.assertIn('│Queued 7',narrow)
        self.assertIn('│3/4 active (shared)',narrow)
        self.assertIn('R0:0 R1:3 R2:2 R3:1',narrow)
        self.assertTrue(all(len(line)==60 for line in narrow.splitlines()))
        self.assertIn('RECENT RUNS',status.render(runs,self.root,width=120))
        pipes[0]['flow']['ready']['cards']=25
        self.assertIn('25 ready · next 20',status.render([],self.root,width=120,pipeline=pipes))
        pipes[0]['workers']=[{'worker':f'r{i}','card':f'Card {i}', 'stage':'review', 'context_cards':2, 'model':'terra', 'handled':3, 'started':0} for i in range(1,6)]
        pipes[0]['events']=[{'name':f'Event {i}','outcome':'confirmed','detail':'','time':i,'run':'r1'} for i in range(8)]
        crowded=status.render([],self.root,width=120,height=40,pipeline=pipes)
        self.assertLessEqual(len(crowded.splitlines()),40)
        self.assertGreaterEqual(sum('Event ' in line for line in crowded.splitlines()),4)
    def test_pipeline_implementer_slots_shared_queue_and_commit_deduplication(self):
        self.queue([{'name':'Legacy','status':'committed'}])
        folder=self.root/'pipeline-example';folder.mkdir()
        queue=folder/'implementation-queue.json'
        queue.write_text(json.dumps({'cards':[{'name':'Legacy','status':'committed','commit':'a'}, {'name':'New','status':'committed','commit':'b'}, {'name':'Gap','status':'generator_gap'}, {'name':'Held','status':'deferred_blocked'}]}))
        (folder/'config.json').write_text(json.dumps({'lane_root':str(self.root/'new-project'), 'implementation_queue':str(queue), 'implementers':{'i1':{},'i2':{}}, 'implementation_cards_per_context':20}))
        (folder/'status.json').write_text(json.dumps({'counts':[], 'active':[], 'implementation':[{'status':'queued','cards':3},{'status':'committed','cards':1}]}))
        for slot, live in [('i1',{'state':'running','card':'New work','context_cards':4,'model':'terra'}),('i2',{'state':'waiting','context_cards':0})]:
            worker=folder/'implementation'/slot;worker.mkdir(parents=True)
            (worker/'live.json').write_text(json.dumps(live))
        pipes=status.pipelines(self.root,status.collect(self.root,{}),active_only=True)
        self.assertEqual(pipes[0]['implementation_committed'],2)
        text=status.render([],self.root,width=120,pipeline=pipes)
        self.assertIn('implement 1/2 active · 3 queued · 1 new committed · 1 generator gaps · 1 deferred',text)
        self.assertRegex(text,r'i1\s+implement\s+WORKING')
        self.assertIn('New work',text)
        self.assertRegex(text,r'i2\s+implement\s+READY')
    def test_review_progress_and_outcomes_replace_stale_implementation_display(self):
        self.queue([{'name':'Old implementation','status':'committed','finished':'2026-01-01T00:00:00Z'}])
        folder=self.root/'pipeline-example';folder.mkdir()
        (folder/'config.json').write_text(json.dumps({'cards_per_context':20}))
        snapshot={'counts':[{'stage':'review','status':'passed','cards':1},{'stage':'confirm','status':'passed','cards':1},{'stage':'review','status':'running','cards':1}],
                  'active':[{'worker':'r1','name':'Current','stage':'review'}], 'worker_progress':[{'worker':'r1','handled':7}],
                  'recent':[{'id':12,'name':'New review','worker':'r1','kind':'passed','at':'2026-01-01T01:00:00Z','detail':{'stage':'confirm'}},
                            {'id':11,'name':'New fix','worker':'r1','kind':'queued','at':'2026-01-01T00:59:00Z','detail':{'stage':'review'}}]}
        (folder/'status.json').write_text(json.dumps(snapshot))
        worker=folder/'workers/r1';worker.mkdir(parents=True)
        (worker/'live.json').write_text(json.dumps({'state':'running','card':'Current','context_cards':0,'model':'terra'}))
        runs=status.collect(self.root,{})
        pipes=status.pipelines(self.root,runs)
        output=status.render(runs*15,self.root,width=120,height=30,event_limit=2,pipeline=pipes)
        self.assertIn('1 LIVE',output)
        self.assertIn('2 REVIEW READY',output)
        self.assertNotIn('cards handled',output)
        self.assertIn('Current',output)
        self.assertNotIn('context 0/20',output)
        self.assertIn('New review · confirmed',output)
        self.assertIn('New fix · fixed → confirmation',output)
        self.assertLess(output.index('New review · confirmed'),output.index('New fix · fixed → confirmation'))
        self.assertLessEqual(len(output.splitlines()),30)
    def test_terminal_defaults_to_color_live_mode_and_restores_screen(self):
        self.queue([])
        master, slave = pty.openpty()
        proc = subprocess.Popen([str(Path(__file__).with_name('status.sh')), '--root', str(self.root)], stdin=slave, stdout=slave, stderr=slave, env={k:v for k,v in os.environ.items() if k!='NO_COLOR'}, start_new_session=True)
        os.close(slave)
        try:
            time.sleep(1.3); proc.send_signal(signal.SIGINT); proc.wait(timeout=5)
            chunks=[]
            while True:
                try:part=os.read(master,65536)
                except OSError:break
                if not part:break
                chunks.append(part)
            text=b''.join(chunks).decode()
            self.assertEqual(proc.returncode,0)
            self.assertGreaterEqual(text.count('CARD LANE'),2)
            self.assertIn('\033[?1049h',text)
            self.assertIn('\033[?1049l',text)
            self.assertIn('\033[1;36m',text)
        finally:
            if proc.poll() is None:proc.kill();proc.wait()
            os.close(master)
    def test_watch_refreshes_and_ctrl_c_exits_cleanly(self):
        self.queue([])
        proc = subprocess.Popen([str(Path(__file__).with_name('status.sh')), '--watch', '0.4', '--root', str(self.root)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            time.sleep(1.3); proc.send_signal(signal.SIGINT)
            out, err = proc.communicate(timeout=5)
        finally:
            if proc.poll() is None:proc.kill(); proc.wait()
        self.assertEqual(proc.returncode, 0, err)
        self.assertGreaterEqual(out.count('CARD LANE'), 2)
        self.assertNotIn('Traceback', err)
    def test_watch_loads_updated_dashboard_code(self):
        self.queue([])
        script=self.root/'dashboard.py'
        original=Path(status.__file__).read_text();script.write_text(original)
        proc=subprocess.Popen(['python3',str(script),'--watch','0.1','--root',str(self.root)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        try:
            time.sleep(0.5)
            script.write_text(original.replace("'CARD LANE', 'title'", "'RELOADED', 'title'"))
            time.sleep(0.6);proc.send_signal(signal.SIGINT)
            out,err=proc.communicate(timeout=5)
        finally:
            if proc.poll() is None:proc.kill();proc.wait()
        self.assertEqual(proc.returncode,0,err)
        self.assertIn('CARD LANE',out)
        self.assertIn('RELOADED',out)

if __name__ == '__main__':unittest.main()
