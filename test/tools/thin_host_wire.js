import Web from '../../src/web/app.bend';
import Mobile from '../../src/mobile/app.bend';
import { plain, learnJson, json } from '../../src/web/wire.js';
learnJson(Mobile.json_tags());
let checks = 0;
function equal(name, actual, expected) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw Error(name + ': unexpected production result');
  console.log('ok ' + name);checks++;
}
const context = {kind:'mail-draft',owner:'b',origin:'epoch',conv:'room:r',recipient:'room:r'};
let ui = Web.mail_local_context(Web.init(0n,'fixture'), Web.presentation_json(JSON.stringify(context)));
ui = Web.mail_local_field(ui,'actual 日本語🙂');
equal('production export snapshot has exact field', JSON.parse(Web.mail_snapshot(ui)).draft,'actual 日本語🙂');
equal('production export identity has exact owner/origin', Web.mail_identity(ui),'@wm:5:epoch1:broom:r:');
equal('production restored text stays text-only',Web.mail_local_text(Web.mail_restore(ui,'{"draft":"saved"}')),'saved');
equal('production invalid local context cannot persist',Web.mail_identity(Web.mail_local_context(ui,Web.presentation_json('{}'))),'');
const session={sel:'t',rid:2,drafts:{},route:{kind:'bot-tab',bot:'b',tab:'mail'}};
const draft=plain(Mobile.session_draft(json(session),'new'));
equal('production native draft keeps owning route',draft.route,session.route);
const choice=plain(Mobile.thin_choice(json(draft),'select','hub|next'));
equal('production selection omits old bot route',Object.prototype.hasOwnProperty.call(choice,'route'),false);
equal('production selection chooses exact current thread',plain(Mobile.thin_target(json(choice))),{kind:'thread',id:'next'});
const partial=plain(Mobile.thin_partial(Mobile.json_parse('{"thread":{"entries":[{"id":"e","text":"prefix"}]}}'),Mobile.json_parse('{"partial":[{"id":"e","version":"9","cursor":"opaque","hasMore":true,"complete":false}]}')));
equal('production native materialization retains partial version',partial.thread.entries[0].partial.version,'9');
equal('production native materialization does not claim complete',partial.thread.entries[0].partial.complete,false);
equal('production current page with no thread stays absent',plain(Mobile.thin_partial(Mobile.json_parse('{"thread":null}'),Mobile.json_parse('{"partial":[]}'))).thread,null);
console.log(JSON.stringify({pass:true,checks,scope:'Actual emitted production Bend export wire only; no service/socket/DOM/device acceptance'}));
