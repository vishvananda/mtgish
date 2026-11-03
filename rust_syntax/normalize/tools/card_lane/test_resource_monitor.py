import copy
import unittest

import resource_monitor as monitor


def snapshot(at):
    return dict(at='2026-11-03T17:20:00+00:00', monotonic=at,
                workers={'r1':dict(role='review', cgroup='/worker', cpu_usec=at*1e6,
                                  throttled_usec=0, memory_bytes=monitor.GIB, anon_bytes=monitor.GIB/2,
                                  shmem_bytes=0)},
                host=dict(cpu_total=at*100, cpu_busy=at*20, cpu_iowait=at*5, cpus=8,
                          affinity_cpus=8, memory_available_bytes=10*monitor.GIB,
                          swap_in_pages=0, swap_out_pages=0, pressure_usec={'cpu_some':0}))


class ResourceTests(unittest.TestCase):
    def test_cpu_is_counter_delta_including_all_cgroup_descendants(self):
        result = monitor.interval(snapshot(1),snapshot(3))
        self.assertEqual(result['seconds'],2)
        self.assertEqual(result['roles']['review']['cpu_cores'],1)
        self.assertEqual(result['host']['cpu_cores'],1.6)
        self.assertEqual(result['host']['iowait_cores'],.4)
        self.assertEqual(result['host']['swap_in_mib_per_second'],0)

    def test_restart_counter_reset_and_missing_worker_are_not_negative_cpu(self):
        previous,current=snapshot(10),snapshot(11)
        current['workers']['r1']['cpu_usec']=0
        result=monitor.interval(previous,current)
        self.assertIsNone(result['workers']['r1']['cpu_cores'])
        self.assertEqual(result['roles']['review']['measured_workers'],0)
        report=monitor.summarize([result])
        self.assertIsNone(report['projection_plus_three_reviewers']['host_cpu_cores']['mean'])
        with self.assertRaises(ValueError):monitor.interval(previous,previous)

    def test_weighted_average_p95_and_projection(self):
        first=monitor.interval(snapshot(1),snapshot(3))
        second=copy.deepcopy(first);second['seconds']=18
        second['roles']['review']['cpu_cores']=3
        second['host']['cpu_cores']=4
        result=monitor.summarize([first,second])
        self.assertEqual(result['sampled_seconds'],20)
        self.assertEqual(result['roles']['review']['cpu_cores']['mean'],2.8)
        self.assertEqual(result['host_cpu_cores']['mean'],3.76)
        self.assertEqual(result['host_cpu_cores']['p95'],4)
        self.assertEqual(result['projection_plus_three_reviewers']['host_cpu_cores']['mean'],12.16)
        self.assertEqual(result['roles']['verify']['workers'],0)

    def test_thirteen_worker_projection_uses_the_target_mix_not_three_more_forever(self):
        sample=monitor.interval(snapshot(1),snapshot(3))
        for name,count,cpu in [('implement',5,5),('review',7,7),('verify',1,.1)]:
            sample['roles'][name]=dict(workers=count,measured_workers=count,cpu_cores=cpu,
                                      memory_bytes=0,anon_bytes=0,shmem_bytes=0,throttled_seconds=0)
        sample['host']['cpu_cores']=20
        result=monitor.summarize([sample])
        self.assertEqual(result['projection_13_workers']['host_cpu_cores']['mean'],20)
        self.assertEqual(result['projection_plus_three_reviewers']['host_cpu_cores']['mean'],23)


if __name__=='__main__':unittest.main()
