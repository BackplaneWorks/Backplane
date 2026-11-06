#!/bin/sh
# On the Mac: pair the app from an invitation file (BACKPLANE_PAIR_LINK, the simulator hook) and log what it does. usage: ios-pair-mac.sh HOST PORT
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}; APP=${APP:-$HOME/bp-gpui-ios/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app}
# from this Mac (a remote tailnet peer): enroll with a second invitation, then try the WebSocket upgrade with the device credential (status codes only)
python3 - <<'PY' > $HOME/bp-gpui-ios/shots/curl-ws.txt 2>&1
import re,subprocess,json,os
link=open(os.path.expanduser('~/bp-gpui-ios/e2e-invite2')).read().strip()
origin=re.match(r'(https?://[^/]+)',link)[1]; secret=re.search(r'invite=([a-f0-9]{32})',link)[1]
en=subprocess.run(['curl','-s','-m8','-w','\\n%{http_code}','-X','POST','-H','Authorization: Bearer '+secret,'-H','X-Backplane-Client: native',origin+'/auth/enroll'],capture_output=True,text=True).stdout
body,code=en.rsplit('\n',1); tok=json.loads(body).get('token','') if code=='200' else ''
print('enroll from the Mac:',code,'token',bool(tok))
cap=subprocess.run(['curl','-s','-m5','-o','/tmp/cap.json','-w','%{http_code}','-H','Authorization: Bearer '+tok,origin+'/client/capabilities'],capture_output=True,text=True).stdout
print('GET /client/capabilities with the device credential ->',cap,open('/tmp/cap.json').read()[:160].replace(tok,'<token>'))
cap0=subprocess.run(['curl','-s','-m5','-o','/dev/null','-w','%{http_code}',origin+'/client/capabilities'],capture_output=True,text=True).stdout
print('GET /client/capabilities with no credential ->',cap0)
for q in ['/ws?token='+tok,'/ws?view=1&mobile=1&token='+tok]:
    r=subprocess.run(['curl','-s','-m5','-o','/dev/null','-w','%{http_code}','--http1.1','-H','Connection: Upgrade','-H','Upgrade: websocket','-H','Sec-WebSocket-Version: 13','-H','Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==',origin+q],capture_output=True,text=True).stdout
    print('ws upgrade',q.split('token=')[0],'->',r)
PY
xcrun simctl boot $U 2>/dev/null; xcrun simctl bootstatus $U -b >/dev/null 2>&1
xcrun simctl terminate $U dev.backplane.gpui 2>/dev/null; xcrun simctl uninstall $U dev.backplane.gpui; xcrun simctl install $U "$APP" || exit 1
export SIMCTL_CHILD_BACKPLANE_PAIR_LINK="$(cat $HOME/bp-gpui-ios/e2e-invite)" SIMCTL_CHILD_BACKPLANE_PERFORMANCE=1
xcrun simctl launch $U dev.backplane.gpui >/dev/null; sleep 25
xcrun simctl io $U screenshot $HOME/bp-gpui-ios/shots/pairlog.png 2>&1 | tail -1
xcrun simctl spawn $U log show --last 40s --predicate 'process == "Backplane"' --style compact 2>/dev/null | grep -E "Backplane (connection|view|screen|navigation|simulator)|push:|pair" | sed 's/^.*Backplane\[[0-9:a-f]*\] ([A-Za-z]*) //' | cut -c1-200 > $HOME/bp-gpui-ios/shots/pairlog.txt
xcrun simctl shutdown $U 2>/dev/null
