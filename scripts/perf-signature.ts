// Builder keys are host configuration, never candidate files. A receipt's
// asserted hashes alone do not authenticate that the configured builder ran.
import {createHash,createPrivateKey,createPublicKey,sign,verify,type KeyObject} from 'node:crypto';
import {readFileSync,realpathSync} from 'node:fs';
import {isAbsolute,relative,sep} from 'node:path';

function externalFile(root:string,name:string) {
  const value=process.env[name];
  if(!value || !isAbsolute(value)) throw new Error(`${name} must name a configured absolute host key path`);
  const path=realpathSync(value),within=relative(realpathSync(root),path);
  if(within==='' || (!within.startsWith('..'+sep) && !isAbsolute(within)))
    throw new Error(`${name} must be outside the candidate checkout`);
  return path;
}
function publicKey(root:string) {
  const key=createPublicKey(readFileSync(externalFile(root,'BACKPLANE_PERF_TRUST_KEY')));
  if(key.asymmetricKeyType!=='ed25519') throw new Error('performance trust key must be Ed25519');
  return key;
}
function fingerprint(key:KeyObject) {
  return createHash('sha256').update(key.export({type:'spki',format:'der'})).digest('hex');
}
function canonical(value:any):string {
  if(value===null || typeof value!=='object') return JSON.stringify(value);
  if(Array.isArray(value)) return '['+value.map(canonical).join(',')+']';
  return '{'+Object.keys(value).sort().map(k=>JSON.stringify(k)+':'+canonical(value[k])).join(',')+'}';
}
export function signingConfiguration(root:string) {
  const trust=publicKey(root);
  const privateKey=createPrivateKey(readFileSync(externalFile(root,'BACKPLANE_PERF_SIGNING_KEY')));
  if(privateKey.asymmetricKeyType!=='ed25519' || fingerprint(createPublicKey(privateKey))!==fingerprint(trust))
    throw new Error('performance builder key does not match configured trust key');
  return {trust,privateKey};
}
export function attestBuild(root:string,record:Record<string,unknown>) {
  const {trust,privateKey}=signingConfiguration(root);
  return {...record,attestation:{public_key_sha256:fingerprint(trust),
    signature:sign(null,Buffer.from(canonical(record)),privateKey).toString('base64')}};
}
export function authenticateBuild(root:string,receipt:any) {
  const {attestation,...record}=receipt;
  const trust=publicKey(root);
  if(!attestation || attestation.public_key_sha256!==fingerprint(trust)
      || typeof attestation.signature!=='string'
      || !verify(null,Buffer.from(canonical(record)),trust,Buffer.from(attestation.signature,'base64')))
    throw new Error('build receipt lacks a valid configured-builder signature');
  return record;
}
if(import.meta.main) {
  try {
    if(process.argv[2]!=='--check-signing-config') throw new Error('usage: perf-signature.ts --check-signing-config');
    signingConfiguration(process.cwd());
  } catch(error) {console.error(`performance signing configuration REJECTED: ${String(error)}`);process.exitCode=1;}
}
