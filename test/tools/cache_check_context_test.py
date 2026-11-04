"""Entry-context/cache migration controls; no actual proof or fake gate claim."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shlex
import shutil
import tempfile
import subprocess
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('entry_fixture',Path(__file__).with_name('content_cache_test.py'))
fixture=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture)
cc=fixture.cc


class Context(fixture.Base):
    fake_bend=fixture.RunCheck.fake_bend
    run_check=fixture.RunCheck.run_check
    plan_file=fixture.RunCheck.plan_file

    def test_old_aggregate_pass_has_no_valid_namespace_or_record_path(self):
        key, body=cc.unit_key('check','src/other.bend',{'src/other.bend':'old'},fixture.TOOL)
        entry={'schema':1,'verdict':'passed','key':key,'body':body,'mac':cc.mac(b'x'*32,key,body)}
        self.assertFalse(cc.valid_entry(entry,key,b'x'*32))
        with self.assertRaisesRegex(cc.CacheError,'retired'):
            cc._record(key,body,self.cache)
        now, current=cc.check_units(self.root)['src/other.bend']
        self.assertEqual(current['gate'],'check-entry-v2')
        self.assertNotEqual(key,now)
        self.assertEqual(current['tool']['check_execution']['argv'],['bend','src/other.bend','--check-only'])

    def test_each_miss_is_a_fresh_exact_entry_no_aggregate_command(self):
        log=self.root.parent/'calls'
        self.fake_bend('echo "$1|$2" >> '+shlex.quote(str(log))+'\necho "All terms check."\n')
        self.assertEqual(self.run_check(self.plan_file()),0)
        calls=log.read_text().splitlines()
        self.assertEqual(calls,[n+'|--check-only' for n in sorted(cc.check_units(self.root))])
        self.assertEqual(len(calls),4)
        self.assertFalse(any('manifest' in n for n in calls))

    def test_entry_main_order_is_part_of_its_key_not_a_user_pass(self):
        self.write('src/L.bend','import Base\nlaw value:\n  {0n == 0n : Nat}\n')
        self.write('src/P.bend','import Base\nimport ./L.bend as L\ndef L.value():\n  {==}\n')
        self.write('src/U.bend','import Base\nimport ./L.bend as L\n')
        first='import Base\nimport ./P.bend as P\nimport ./U.bend as U\n'
        second='import Base\nimport ./U.bend as U\nimport ./P.bend as P\n'
        self.write('src/MAIN.bend',first)
        units=cc.check_units(self.root);main=units['src/MAIN.bend'][0];user=units['src/U.bend'][0]
        self.write('src/MAIN.bend',second)
        updated=cc.check_units(self.root)
        self.assertNotEqual(main,updated['src/MAIN.bend'][0])
        self.assertNotEqual(main,user)
        # U's own entry is unchanged; it is never blessed by MAIN's context.
        self.assertEqual(user,updated['src/U.bend'][0])
        self.assertEqual(updated['src/U.bend'][1]['tool']['check_execution']['argv'][1],'src/U.bend')

    def test_other_module_can_never_supply_a_pass_for_failing_standalone_entry(self):
        # Shell fixture only: it exposes exact argv and refuses the selected
        # entry. It is not used as mathematical/actual Bend qualification.
        self.write('src/P.bend','import Base\n')
        self.write('src/U.bend','import Base\n')
        self.fake_bend('case "$1" in src/U.bend) echo "Error: unfilled law"; exit 1;; esac\necho "All terms check."\n')
        self.assertEqual(self.run_check(self.plan_file()),1)
        hits=cc.plan(cc.check_units(self.root),local=self.cache)[0]
        self.assertNotIn('src/U.bend',hits)
        self.assertIn('src/P.bend',hits)  # Genuine scoped passes before failure survive.

    def test_jsc_or_stack_context_change_invalidates_entry_key(self):
        before=cc.check_units(self.root)['src/other.bend'][0]
        with patch.dict(os.environ,{'BUN_JSC_maxPerThreadStackUsage':'12345'}):
            self.assertNotEqual(before,cc.check_units(self.root)['src/other.bend'][0])
        with patch.object(cc.resource,'getrlimit',return_value=(12345,12345)):
            self.assertNotEqual(before,cc.check_units(self.root)['src/other.bend'][0])

    def test_warm_cli_is_advisory_three_not_green_gate(self):
        self.mark_all_passed()
        plan=self.plan_file()
        output=io.StringIO()
        with contextlib.redirect_stdout(output):
            status=cc.main(['--root',str(self.root),'run-check','--plan',str(plan)])
        self.assertEqual(status,3)
        result=json.loads(output.getvalue())
        self.assertEqual(result['status'],'pass-standalone')
        self.assertIs(result['gate_qualified'],False)

    def test_cli_standalone_failure_is_distinct_and_preserves_message(self):
        self.fake_bend('echo "Error: unfilled law"; exit 1\n')
        plan=self.plan_file();output=io.StringIO();errors=io.StringIO()
        guard=type('Guard',(),{'inherited_lock':staticmethod(lambda path:3),'lock_path':staticmethod(lambda:Path('/TEST-ONLY')),'admit':staticmethod(lambda profile:True)})()
        with patch.object(cc,'_module',return_value=guard),contextlib.redirect_stdout(output),contextlib.redirect_stderr(errors):
            status=cc.main(['--root',str(self.root),'run-check','--plan',str(plan)])
        self.assertEqual(status,4)
        self.assertEqual(json.loads(output.getvalue())['status'],'inconclusive-standalone')
        self.assertIn('Error: unfilled law',errors.getvalue())
        self.assertEqual(cc.plan(cc.check_units(self.root),local=self.cache)[0],[])


class ShellAdvisory(unittest.TestCase):
    def test_old_zero_helper_cannot_make_shell_report_gate_success(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);(root/'scripts').mkdir();(root/'bin').mkdir()
            shutil.copyfile(Path(__file__).resolve().parents[2]/'scripts/cached-check.sh',root/'scripts/cached-check.sh')
            (root/'scripts/bendok.sh').write_text('')
            (root/'scripts/heavy-guard.sh').write_text('heavy_guard() { echo BAD-GUARD; return 99; }\n')
            program=root/'bin/python3'
            program.write_text("""#!/bin/sh
case "$1:$2" in
 scripts/laws-shape.py:*) exit 0 ;;
 scripts/content-cache.py:plan-check) echo '{"hit":["x.bend"],"miss":[]}'; exit 0 ;;
 scripts/content-cache.py:run-check) echo LEGACY-ZERO; exit 0 ;;
 -c:*) exit 0 ;;
esac
exit 98
""")
            program.chmod(0o755)
            env={**os.environ,'PATH':str(root/'bin')+os.pathsep+os.environ['PATH']}
            result=subprocess.run(['sh','scripts/cached-check.sh'],cwd=root,env=env,text=True,capture_output=True)
            self.assertEqual(result.returncode,3,result)
            self.assertIn('standalone pre-check only',result.stdout)
            self.assertNotIn('BAD-GUARD',result.stdout)

    def test_miss_shell_has_no_unbound_variables_and_distinguishes_failure(self):
        for status in (3,4):
            with self.subTest(status=status),tempfile.TemporaryDirectory() as temp:
                root=Path(temp);(root/'scripts').mkdir();(root/'bin').mkdir()
                shutil.copyfile(Path(__file__).resolve().parents[2]/'scripts/cached-check.sh',root/'scripts/cached-check.sh')
                (root/'scripts/bendok.sh').write_text('')
                (root/'scripts/heavy-guard.sh').write_text('heavy_guard() { echo MOCK-GUARD; return 0; }\n')
                program=root/'bin/python3'
                program.write_text('''#!/bin/sh
case "$1:$2" in
 scripts/laws-shape.py:*) exit 0 ;;
 scripts/content-cache.py:plan-check) echo '{"hit":[],"miss":["a.bend"]}'; exit 0 ;;
 scripts/content-cache.py:run-check) echo SCOPED; exit '''+str(status)+''' ;;
 -c:*) echo a.bend; exit 0 ;;
esac
exit 98
''')
                program.chmod(0o755)
                env={**os.environ,'PATH':str(root/'bin')+os.pathsep+os.environ['PATH']}
                result=subprocess.run(['sh','scripts/cached-check.sh'],cwd=root,env=env,text=True,capture_output=True)
                self.assertEqual(result.returncode,status,result)
                self.assertIn('MOCK-GUARD',result.stdout)
                self.assertIn('SCOPED',result.stdout)
                self.assertNotIn('parameter not set',result.stderr)


if __name__=='__main__':unittest.main()
