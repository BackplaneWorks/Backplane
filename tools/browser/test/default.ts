import { defaultEngine } from '../src/session.ts';
import { check } from '../../cef/test/harness.ts';
check('Linux X11 helper defaults to CEF', defaultEngine(null, 123, ':96', 'linux') === 'cef');
check('explicit Chrome remains Chrome', defaultEngine('chrome', 123, ':96', 'linux') === 'chrome');
check('explicit CEF remains CEF', defaultEngine('cef', 123, ':96', 'linux') === 'cef');
check('headless uses Chrome frames', defaultEngine(null, 0, ':96', 'linux') === 'chrome');
check('missing X display uses Chrome frames', defaultEngine(null, 123, '', 'linux') === 'chrome');
check('non-Linux uses Chrome frames', defaultEngine(null, 123, ':96', 'darwin') === 'chrome');
let invalid = false;
try { defaultEngine('invalid', 123, ':96', 'linux'); } catch { invalid = true; }
check('invalid engine is refused', invalid);
