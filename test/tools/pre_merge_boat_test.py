"""Boat-run pre-merge guard controls. Never execute this suite locally."""
import copy
import fnmatch
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace

HERE=Path(__file__).resolve().parent
PATH=HERE/'pre-merge-boat.py' if (HERE/'pre-merge-boat.py').exists() else HERE.parents[1]/'scripts/pre-merge-boat.py'
spec=importlib.util.spec_from_file_location('premerge_boat_controls',PATH)
pm=importlib.util.module_from_spec(spec);spec.loader.exec_module(pm)


def setUpModule():
    # Enforce the user rule for accidental direct unittest invocations too.
    if (os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX') != '1'
            or os.environ.get('BACKPLANE_HEAVY_OFFLOADED') != '1'
            or os.environ.get('BEND_NO_TELEMETRY') != '1'):
        raise RuntimeError('pre-merge controls are Boat-only; never run locally')
    group=Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::',1)[1].strip().lstrip('/')
    maximum=(group/'memory.max').read_text().strip()
    if maximum=='max' or not 0<int(maximum)<=3700000000 or (group/'memory.swap.max').read_text().strip()!='0':
        raise RuntimeError('pre-merge controls need actual <=3.7GB/swap0 cgroup')


class Contracts(unittest.TestCase):
    def resources(self):
        return {'memory_max_bytes':pm.CAP,'swap_max_bytes':0,'peak_bytes':10000000,
                'events':{'max':0,'oom':0,'oom_kill':0,'oom_group_kill':0}}

    def report(self):
        return {'schema':3,'kind':'boat-merged-laws-shape','head':'a'*40,'base':'b'*40,'tree':'c'*40,
                'guard_files':{name:{'base':{'mode':'100644','sha256':'8'*64},'head':{'mode':'100644','sha256':'8'*64}} for name in pm.GUARD_FILES},
                'verification_base':'b'*40,'merge_driver_sha256':'2'*64,'guard_files_match_base':True,'guard_change_ack':'','controls_exit':0,'controls_sha256':'6'*64,'controls_source_sha256':'7'*64,'worker_sha256':'d'*64,'state':'passed','shape_exit':0,'laws_sha256':'e'*64,
                'proof_sha256':'f'*64,'shape_sha256':'1'*64,'resources':self.resources()}

    def test_only_closed_remote_ref_or_complete_object_id(self):
        self.assertEqual(pm.base_ref('origin/main'),('refs/heads/main','refs/remotes/origin/main'))
        self.assertEqual(pm.base_ref('a'*40),('a'*40,None))
        for bad in ('main','origin/../main','origin/-upload','origin/main;false','origin/a//b','a'*39,'origin/'):
            with self.subTest(base=bad),self.assertRaises(ValueError):pm.base_ref(bad)

    def test_no_local_worker_or_telemetry_fallback(self):
        with patch.dict(os.environ,{},clear=True),patch.object(pm.subprocess,'run') as run:
            with self.assertRaisesRegex(ValueError,'Boat-only'):pm.worker_group()
            run.assert_not_called()

    def test_real_cap_and_oom_evidence_required(self):
        self.assertTrue(pm.resource_ok(self.resources()))
        for fields in ({'memory_max_bytes':8*1024**3},{'swap_max_bytes':1},{'peak_bytes':0},
                       {'memory_max_bytes':True},{'events':{}},{'events':{'oom':0,'oom_kill':1,'oom_group_kill':0}}):
            self.assertFalse(pm.resource_ok({**self.resources(),**fields}),fields)

    def test_owned_receipt_requires_exact_head_worker_and_shape_success(self):
        good=self.report();self.assertTrue(pm.receipt_ok(good,'a'*40,'d'*64))
        for fields in ({'state':'failed'},{'shape_exit':1},{'shape_exit':False},{'schema':True},{'schema':1},{'schema':2},{'controls_exit':1},{'controls_exit':False},{'controls_sha256':None},{'verification_base':'0'*40},{'merge_driver_sha256':None},
                       {'head':'0'*40},{'worker_sha256':'0'*64},{'tree':'not-a-tree'},{'proof_sha256':None}):
            self.assertFalse(pm.receipt_ok({**good,**fields},'a'*40,'d'*64),fields)
        for missing in ('resources','laws_sha256','proof_sha256','tree','base'):
            bad=copy.deepcopy(good);bad.pop(missing)
            self.assertFalse(pm.receipt_ok(bad,'a'*40,'d'*64),missing)

    def test_dispatch_has_no_local_merge_tree_or_shape_command(self):
        # Regression for the hard Boat-only rule. Dispatch may read Git IDs,
        # transport a pushed HEAD and compare byte/ref identities only.
        import inspect
        source=inspect.getsource(pm.dispatch)
        self.assertNotIn("'merge-tree'",source)
        self.assertNotIn('inspect_merge(',source)
        self.assertNotIn("'python3', 'scripts/laws-shape.py'",source)
        self.assertIn("SCOPE_CMD",source)
        self.assertIn("systemd-run --user --scope",pm.SCOPE_CMD)
        self.assertIn("MemoryMax=536870912",pm.SCOPE_CMD)
        self.assertIn("MemorySwapMax=0",pm.SCOPE_CMD)
        self.assertIn("'--no-setup-bend'",source)

    def test_guard_matrix_and_reviewed_state_cannot_be_omitted_or_mislabelled(self):
        good=self.report()
        for value in (None,{},{'extra':{}}, {name:{'base':None,'head':None} for name in pm.GUARD_FILES}):
            self.assertFalse(pm.receipt_ok({**good,'guard_files':value},'a'*40,'d'*64))
        changed=copy.deepcopy(good)
        changed['guard_files']['scripts/boat-job.sh']['head']['sha256']='9'*64
        changed.update(guard_files_match_base=False,guard_change_ack='a'*40,state='passed-reviewed-guard-change')
        self.assertFalse(pm.receipt_ok(changed,'a'*40,'d'*64))
        self.assertTrue(pm.receipt_ok(changed,'a'*40,'d'*64,'a'*40))
        self.assertFalse(pm.receipt_ok({**changed,'guard_files_match_base':True,'state':'passed'},'a'*40,'d'*64,'a'*40))
        self.assertFalse(pm.receipt_ok({**changed,'state':'passed'},'a'*40,'d'*64,'a'*40))
        self.assertFalse(pm.receipt_ok(changed,'a'*40,'d'*64,'a'*39))
        self.assertFalse(pm.receipt_ok({**good,'guard_change_ack':'a'*40},'a'*40,'d'*64,'a'*40))


class RunnerTransport(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='premerge-runner-control-')
        self.addCleanup(self.temp.cleanup)
        self.path=Path(self.temp.name)/'runner.sh'
        self.data=b'#!/bin/sh\n# source fixture only; never a proof producer\nexit 0\n'
        self.path.write_bytes(self.data);self.path.chmod(0o755)
        self.digest=pm.sha(self.data)

    def test_explicit_runner_requires_absolute_path_and_complete_digest(self):
        for path,digest in (('runner.sh',self.digest),(str(self.path),''),(str(self.path),self.digest[:12]),(str(self.path),'z'*64)):
            with self.subTest(path=path,digest=digest),self.assertRaises(ValueError):
                pm.runner_source(path,digest,'exclude')
        record,data=pm.runner_source(str(self.path),self.digest,'exclude')
        self.assertEqual(data,self.data);self.assertEqual(record['sha256'],self.digest)

    def test_wrong_or_changed_runner_bytes_are_refused(self):
        self.path.write_bytes(self.data+b'# changed\n')
        with self.assertRaisesRegex(ValueError,'SHA256'):pm.runner_source(str(self.path),self.digest,'exclude')

    def test_runner_symlink_is_refused(self):
        link=Path(self.temp.name)/'link';link.symlink_to(self.path)
        with self.assertRaisesRegex(ValueError,'regular'):pm.runner_source(str(link),self.digest,'exclude')

    def test_nonexecutable_or_directory_is_refused(self):
        self.path.chmod(0o644)
        for path in (self.path,Path(self.temp.name)):
            with self.subTest(path=path),self.assertRaisesRegex(ValueError,'regular'):
                pm.runner_source(str(path),self.digest,'exclude')

    def test_implicit_runner_cannot_receive_ambient_overrides(self):
        for digest,mode in ((self.digest,'include-file'),('','exclude')):
            with self.assertRaisesRegex(ValueError,'explicit'):pm.runner_source(None,digest,mode)
        with self.assertRaisesRegex(ValueError,'unsupported'):pm.runner_source(str(self.path),self.digest,'unknown')

    def test_old_exclusion_transport_keeps_only_exact_worker(self):
        worker='scripts/pre-merge-boat.py'
        names=[worker,'PROOF.bend','.env','src/a.bend','scripts/boat-job.sh','scripts/nested/file.py',
               'scripts/name[2].py','odd[1]/file','star*/file','question?/file','CI-OTHER.py']
        args=pm.transport_arguments('exclude',names)
        self.assertTrue(all(args[i]=='--exclude' for i in range(0,len(args),2)))
        kept=[name for name in names if not any(fnmatch.fnmatch(name,p) for p in args[1::2])]
        self.assertEqual(kept,[worker])
        self.assertEqual(pm.transport_arguments('include-file',[]),['--include-file',worker])

    def test_missing_worker_unsafe_inventory_and_unknown_mode_refuse(self):
        for mode,names in (('exclude',[]),('unknown',[]),('exclude',['scripts/pre-merge-boat.py','../x']),
                           ('exclude',['scripts/pre-merge-boat.py','a\nb'])):
            with self.subTest(mode=mode,names=names),self.assertRaises(ValueError):pm.transport_arguments(mode,names)

    def test_private_snapshot_pins_execution_bytes_and_detects_original_drift(self):
        record,data=pm.runner_source(str(self.path),self.digest,'exclude')
        def start(argv,**kwargs):
            self.assertEqual(Path(argv[0]).read_bytes(),self.data)
            self.assertNotEqual(argv[0],str(self.path))
            self.path.write_bytes(self.data+b'# changed while productive\n')
            self.assertEqual(Path(argv[0]).read_bytes(),self.data)
            return SimpleNamespace(stdout=[],wait=lambda:0)
        with patch.object(pm.subprocess,'Popen',side_effect=start):
            self.assertEqual(pm.run_runner(record,data,[]),(0,None,None))
        with self.assertRaisesRegex(ValueError,'SHA256'):pm.runner_source(str(self.path),self.digest,'exclude')

    def test_snapshot_hash_mismatch_refuses_before_start(self):
        record,data=pm.runner_source(str(self.path),self.digest,'exclude')
        with patch.object(pm.subprocess,'Popen') as start:
            with self.assertRaisesRegex(ValueError,'snapshot'):pm.run_runner(record,data+b'bad',[])
            start.assert_not_called()

    def test_ambiguous_identity_waits_for_owned_cleanup_then_refuses(self):
        record,data=pm.runner_source(str(self.path),self.digest,'exclude');waited=[]
        process=SimpleNamespace(stdout=['job j20261104000000-1: logs and artifacts in /tmp/j20261104000000-1\n',
                                        'job j20261104000000-2: logs and artifacts in /tmp/j20261104000000-2\n'],
                                wait=lambda:waited.append(True) or 0)
        with patch.object(pm.subprocess,'Popen',return_value=process):
            with self.assertRaisesRegex(ValueError,'multiple executor'):pm.run_runner(record,data,[])
        self.assertEqual(waited,[True])


class ActualGitMerges(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='premerge-control-');self.addCleanup(self.temp.cleanup)
        self.repo=Path(self.temp.name)/'repo';self.repo.mkdir()
        pm.git(self.repo,'init','-q')
        pm.git(self.repo,'config','user.name','i2cjak')
        pm.git(self.repo,'config','user.email','i2cjak@users.noreply.github.com')
        self.laws=b'import Base\n\nlaw one:\n  {0n == 0n : Nat}\n'
        self.proof=b'import Base\nimport ./LAWS.bend as Laws\n\ndef Laws.one():\n  {==}\n'
        (self.repo/'LAWS.bend').write_bytes(self.laws);(self.repo/'PROOF.bend').write_bytes(self.proof)
        (self.repo/'other.txt').write_text('base\n')
        (self.repo/'scripts').mkdir()
        for name in pm.GUARD_FILES:(self.repo/name).write_text('# base guard source fixture\n')
        source_dir=HERE if (HERE/'laws-shape.py').exists() else HERE.parents[1]/'scripts'
        (self.repo/'scripts/laws-shape.py').write_bytes((source_dir/'laws-shape.py').read_bytes())
        (self.repo/'scripts/merge-laws.py').write_bytes((source_dir/'merge-laws.py').read_bytes())
        self.base=self.commit()

    def commit(self):
        pm.git(self.repo,'add','.');pm.git(self.repo,'commit','-qm','premerge control')
        return pm.git(self.repo,'rev-parse','HEAD').decode().strip()

    def test_actual_merged_tree_not_uncommitted_checkout_bytes(self):
        (self.repo/'other.txt').write_text('ours\n');head=self.commit()
        (self.repo/'LAWS.bend').write_text('UNCOMMITTED MUST NOT BE CHECKED\n')
        tree,laws,proof=pm.inspect_merge(self.repo,self.base,head)
        self.assertRegex(tree,r'^[0-9a-f]{40}$');self.assertEqual(laws,self.laws);self.assertEqual(proof,self.proof)

    def test_conflict_outside_laws_refuses_before_shape(self):
        (self.repo/'other.txt').write_text('ours\n');ours=self.commit()
        pm.git(self.repo,'checkout','-q',self.base)
        (self.repo/'other.txt').write_text('theirs\n');theirs=self.commit()
        with self.assertRaisesRegex(ValueError,'git operation failed'):pm.inspect_merge(self.repo,theirs,ours)

    def test_missing_merged_proof_is_not_a_pass(self):
        (self.repo/'PROOF.bend').unlink();head=self.commit()
        with self.assertRaises(ValueError):pm.inspect_merge(self.repo,self.base,head)

    def test_head_always_pass_checker_cannot_bless_broken_merged_law(self):
        (self.repo/'LAWS.bend').write_text('import Base\n\nlaw one:\n\n')
        (self.repo/'scripts/laws-shape.py').write_text('raise SystemExit(0)\n')
        head=self.commit()
        shape,_=pm.trusted_verifiers(self.repo,self.base,Path(self.temp.name)/'trusted')
        _,laws,proof=pm.inspect_merge(self.repo,self.base,head)
        (Path(self.temp.name)/'LAWS.bend').write_bytes(laws)
        (Path(self.temp.name)/'PROOF.bend').write_bytes(proof)
        result=subprocess.run([os.sys.executable,'-IB',str(shape),str(Path(self.temp.name)/'LAWS.bend'),str(Path(self.temp.name)/'PROOF.bend')],capture_output=True)
        self.assertNotEqual(result.returncode,0)
        self.assertNotEqual(shape.read_bytes(),(self.repo/'scripts/laws-shape.py').read_bytes())

    def test_head_take_ours_driver_cannot_hide_law_conflict(self):
        (self.repo/'LAWS.bend').write_bytes(self.laws.replace(b'0n == 0n',b'1n == 1n'))
        (self.repo/'scripts/merge-laws.py').write_text('raise SystemExit(0)\n')
        head=self.commit()
        pm.git(self.repo,'checkout','-q',self.base)
        (self.repo/'LAWS.bend').write_bytes(self.laws.replace(b'0n == 0n',b'2n == 2n'))
        main=self.commit()
        pm.trusted_verifiers(self.repo,main,Path(self.temp.name)/'trusted')
        with self.assertRaises(ValueError):pm.inspect_merge(self.repo,main,head)

    def test_guard_change_needs_explicit_exact_head_acknowledgement(self):
        _,same=pm.guard_files(self.repo,self.base,self.base)
        self.assertTrue(same)
        (self.repo/'scripts/pre-merge-boat.py').write_text('# changed guard\n')
        head=self.commit()
        matrix,same=pm.guard_files(self.repo,self.base,head)
        self.assertFalse(same)
        self.assertNotEqual(matrix['scripts/pre-merge-boat.py']['base'],matrix['scripts/pre-merge-boat.py']['head'])
        for ack in ('','0'*40,head[:12],'origin/main'):
            self.assertFalse(pm.guard_authorized(head,same,ack),ack)
        self.assertTrue(pm.guard_authorized(head,same,head))

    def test_mode_only_guard_change_requires_acknowledgement(self):
        (self.repo/'scripts/boat-job.sh').chmod(0o755)
        head=self.commit()
        matrix,same=pm.guard_files(self.repo,self.base,head)
        row=matrix['scripts/boat-job.sh']
        self.assertEqual(row['base']['sha256'],row['head']['sha256'])
        self.assertNotEqual(row['base']['mode'],row['head']['mode'])
        self.assertFalse(same);self.assertFalse(pm.guard_authorized(head,same,''))
        self.assertTrue(pm.guard_authorized(head,same,head))

    def test_guard_missing_at_base_is_an_explicit_guard_change(self):
        (self.repo/'scripts/pre-merge-boat.py').unlink()
        missing=self.commit()
        (self.repo/'scripts/pre-merge-boat.py').write_text('# newly introduced guard\n')
        head=self.commit()
        matrix,same=pm.guard_files(self.repo,missing,head)
        self.assertIsNone(matrix['scripts/pre-merge-boat.py']['base'])
        self.assertFalse(same);self.assertTrue(pm.guard_matrix_ok(matrix,False))
        self.assertFalse(pm.guard_authorized(head,same,''))
        self.assertTrue(pm.guard_authorized(head,same,head))


if __name__=='__main__':unittest.main()
