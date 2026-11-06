// Frame evidence from a screen recording (x11grab on Boat, simctl recordVideo on the Mac, adb screenrecord).
// Honest limits: the recording's own frame rate bounds the resolution (x11grab 30 fps = 33 ms), and a changed frame is what the screen showed,
// not what the GPU presented; so click acks derived here are "video-derived", and a budget tighter than the resolution is reported unknown.
import {execFileSync,spawnSync} from 'node:child_process';
// timestamps (seconds, as ffmpeg prints them) of frames that differ from the previous frame by more than `threshold` (scene score 0..1)
export function changedFrames(mp4,threshold=0.0015){
  // showinfo writes to stderr whatever the exit status, so spawnSync (execFileSync drops stderr on success)
  const r=spawnSync('ffmpeg',['-hide_banner','-i',mp4,'-vf',`select='gt(scene,${threshold})',showinfo`,'-f','null','-'],{encoding:'utf8',maxBuffer:1<<28});
  if(r.error)throw r.error;return parse(r.stderr)}
export const parse=txt=>[...String(txt).matchAll(/pts_time:([0-9.]+)/g)].map(m=>Number(m[1]));
// recording rate of a file (frames per second), for the resolution note
export function fps(mp4){try{const o=execFileSync('ffprobe',['-v','error','-select_streams','v:0','-show_entries','stream=avg_frame_rate','-of','csv=p=0',mp4],{encoding:'utf8'}).trim();const[a,b]=o.split('/').map(Number);return b?a/b:a}catch{return NaN}}
// ack times: for each inject epoch (ms) the first changed frame at or after it. `changedEpochMs` are absolute (ms). Taps with no later frame get NaN.
export function acks(changedEpochMs,injectEpochMs){const c=[...changedEpochMs].sort((a,b)=>a-b);return injectEpochMs.map(t=>{const f=c.find(x=>x>=t);return f===undefined?NaN:f-t})}
// changed-frame epoch (ms) from ffmpeg seconds: wallclock recordings carry epoch seconds already; otherwise add the video's start epoch
export const toEpochMs=(secs,videoStartEpochMs=null)=>secs.map(s=>videoStartEpochMs==null?s*1000:videoStartEpochMs+s*1000);
