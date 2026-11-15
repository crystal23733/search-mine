// Synthetic test data only. This encoder is never imported by the web application.
import {
  GOOGLE_CMP_ID, GOOGLE_VENDOR_ID, GOOGLE_AD_PURPOSES,
  GOOGLE_ANALYTICS_PURPOSES, GOOGLE_MEASUREMENT_PURPOSES,
} from '../../packages/protocol/src/index.ts';
function segment(parts) {
  let bits='';
  for(const [value,width] of parts) bits+=BigInt(value).toString(2).padStart(width,'0');
  bits=bits.padEnd(Math.ceil(bits.length/6)*6,'0');
  const alphabet='ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';
  return Array.from({length:bits.length/6},(_,i)=>alphabet[Number.parseInt(bits.slice(i*6,i*6+6),2)]).join('');
}
const mask=(ids,width)=>ids.reduce((value,id)=>value|(1n<<BigInt(width-id)),0n);
export function syntheticGoogleConsent(now=1800000000, choices={advertising:true,analytics:true,measurement:true}) {
  const purposes=choices.advertising?GOOGLE_AD_PURPOSES:[];
  const publisher=[...new Set([...(choices.analytics?GOOGLE_ANALYTICS_PURPOSES:[]),...(choices.measurement?GOOGLE_MEASUREMENT_PURPOSES:[])])];
  const core=segment([
    [2,6],[(now-10)*10,36],[(now-1)*10,36],[GOOGLE_CMP_ID,12],[1,12],[0,6],[0,12],[185,12],[5,6],[1,1],[0,1],[0,12],
    [mask(purposes,24),24],[0,24],[0,1],[0,12],
    [GOOGLE_VENDOR_ID,16],[1,1],[choices.advertising?1:0,12],
    ...(choices.advertising?[[0,1],[GOOGLE_VENDOR_ID,16]]:[]),[0,16],[0,1],[0,12],
  ]);
  const own=segment([[3,3],[mask(publisher,24),24],[0,24],[0,6]]);
  const flags=ids=>Object.fromEntries(ids.map(id=>[id,true]));
  return {cmpId:GOOGLE_CMP_ID,cmpStatus:'loaded',eventStatus:'tcloaded',isServiceSpecific:true,gdprApplies:true,tcfPolicyVersion:5,
    tcString:`${core}.${own}`,purpose:{consents:flags(purposes)},vendor:{consents:{[GOOGLE_VENDOR_ID]:choices.advertising}},publisher:{consents:flags(publisher),restrictions:{}}};
}
