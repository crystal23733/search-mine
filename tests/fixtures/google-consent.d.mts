export function syntheticGoogleConsent(now?: number, choices?: {advertising:boolean;analytics:boolean;measurement:boolean}): {
  cmpId:number;cmpStatus:string;eventStatus:string;isServiceSpecific:boolean;gdprApplies:boolean;tcfPolicyVersion:number;
  tcString:string;purpose:{consents:Record<number,boolean>};vendor:{consents:Record<number,boolean>};publisher:{consents:Record<number,boolean>;restrictions:Record<string,unknown>};
};
