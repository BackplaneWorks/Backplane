import json
from pathlib import Path
R=Path('<capture-root>')
home=R/'run/A-capture/home'
if (home/'events.jsonl').exists(): raise RuntimeError('Use a fresh empty test home; never reseed a running hub.')
home.mkdir(parents=True, exist_ok=True)
(home.parent/'fakehome').mkdir(exist_ok=True)
(home.parent/'agents').mkdir(exist_ok=True)
ev=[]
def add(kind,**kw): ev.append({'$':kind,**kw,'at':1793548800+len(ev)})
for key,value in [('onboarded','1'),('onboard.done','1'),('onboard.uber','1'),('restart.continue','off'),('order.enabled','off'),('telemetry.enabled','off'),('kicad.ipc','on')]: add('SettingSet',key=key,value=value)
add('ProjectCreated',id='royalblue',title='RoyalBlue54L Feather',root=str(R/'projects/royalblue54L_feather'))
add('ProjectCreated',id='cm5',title='CM5 carrier',root=str(R/'projects/cm5-carrier'))
add('ThreadCreated',id='board-demo',project='royalblue',title='Board inspection',env='local',branch='',worktree='',provider='codex',model='gpt-6.1-sol')
add('SettingSet',key='effort.board-demo',value='high')
add('ThreadCreated',id='case-demo',project='cm5',title='3D-printable case',env='local',branch='',worktree='',provider='claude',model='claude-sonnet-5')
add('MessagePosted',thread='case-demo',msg='case-request',role='user',text='Design a two-part 3D-printable case around the CM5 carrier, with openings for every front connector, and show it on Mech.')
add('MessagePosted',thread='case-demo',msg='case-reply',role='assistant',text="The case assembly is on the **Mech** tab, exploded so you can see the board inside:\n\n- Two printable parts: a bottom tray and a vented lid, split at the top of the PCB.\n- Front openings for UART USB-C, microSD, RJ45, USB-C PD and the barrel jack.\n- Flexure buttons for POWER and BOOT, with light pipes above the four LEDs.\n- Board mounting standoffs and 16 mm of headroom for a low CM5 heatsink.")
add('TurnChanged',thread='case-demo',state='completed')
p=R/'repo/docs/media/readme/source/A-demo-events.jsonl'
p.write_text(''.join(json.dumps(e)+'\n' for e in ev))
(R/'run/A-capture/home/events.jsonl').write_bytes(p.read_bytes())
