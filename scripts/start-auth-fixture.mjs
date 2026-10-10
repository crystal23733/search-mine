// Test-only fixed loopback supervisor. The control accepts no commands, paths or state.
import { spawn } from "node:child_process";
import { once } from "node:events";
import http from "node:http";
import { resolve } from "node:path";
import { setTimeout as pause } from "node:timers/promises";
const root = resolve(import.meta.dirname, "..");
let worker, restarting = false, stopping = false;
const server = http.createServer(async (req, res) => {
  res.setHeader("cache-control", "no-store");
  if (req.method === "GET" && req.url === "/__fixture/ready") {
    res.writeHead(worker && worker.exitCode === null && !restarting ? 200 : 503);
    res.end("test-only");
    return;
  }
  if (req.method !== "POST" || req.url !== "/__fixture/restart" || req.headers["transfer-encoding"] || Number(req.headers["content-length"] ?? 0) !== 0) {
    res.writeHead(400);res.end();return;
  }
  if (restarting || stopping) {res.writeHead(409);res.end();return;}
  restarting = true;
  try {
    const response = await fetch("http://127.0.0.1:3001/__fixture/clock", {redirect:"error",signal:AbortSignal.timeout(2000)});
    if (!response.ok) throw Error("Fixture clock unavailable");
    const {utc:clock}=await response.json();
    if (!Number.isSafeInteger(clock) || clock < 0 || clock > 253402300799) throw Error("Invalid fixture clock");
    const before_pid=worker.pid, old=worker;
    const exited=waitExit(old,4000);
    if (!old.kill("SIGKILL")) throw Error("OS kill failed");
    const [exit_code,exit_signal]=await exited;
    await start(clock);
    if (worker.pid === before_pid) throw Error("Fixture PID did not change");
    res.writeHead(200,{"content-type":"application/json"});
    res.end(JSON.stringify({before_pid,after_pid:worker.pid,killed:true,exit_code,exit_signal,clock,ready:true}));
  } catch (error) {
    res.writeHead(503);res.end();
    process.stderr.write(`Fixture restart failed: ${error.message}\n`);
    void shutdown(1);
  } finally {restarting=false;}
});
function waitExit(child,milliseconds) {
  if(child.exitCode!==null || child.signalCode!==null) return Promise.resolve([child.exitCode,child.signalCode]);
  return new Promise((resolveExit,reject)=>{
    const timer=setTimeout(()=>{cleanup();reject(Error("Fixture exit deadline"))},milliseconds);
    const exit=(code,signal)=>{cleanup();resolveExit([code,signal])};
    const error=(failure)=>{cleanup();reject(failure)};
    function cleanup(){clearTimeout(timer);child.off("exit",exit);child.off("error",error)}
    child.once("exit",exit);child.once("error",error);
  });
}
async function start(clock) {
  const binary=resolve(root,process.env.CARGO_TARGET_DIR||"target","debug/examples",process.platform==="win32"?"auth_fixture.exe":"auth_fixture");
  worker=spawn(binary,[],{cwd:root,stdio:"inherit",windowsHide:true,env:{...process.env,...(clock===undefined?{}:{LIAR_FIXTURE_UTC:String(clock)})}});
  const child=worker;
  child.on("error",error=>{process.stderr.write(`Fixture spawn failed: ${error.message}\n`);void shutdown(1)});
  child.on("exit",()=>{if(child===worker&&!restarting&&!stopping)void shutdown(1)});
  const deadline=performance.now()+8000;
  while(performance.now()<deadline) {
    if(child.exitCode!==null||child.signalCode!==null||stopping)throw Error("Fixture exited before readiness");
    try {
      const response=await fetch("http://127.0.0.1:3001/__fixture/ready",{redirect:"error",signal:AbortSignal.timeout(1000)});
      if(response.ok)return;
    } catch { /* Startup connection refusal is expected until the listener binds. */ }
    await pause(25);
  }
  throw Error("Fixture readiness deadline");
}
async function shutdown(code) {
  if(stopping)return;stopping=true;
  if(server.listening)server.close();
  if(worker && worker.exitCode===null && worker.signalCode===null) {
    const exited=waitExit(worker,2500);
    worker.kill("SIGTERM");
    try{await exited}catch{const killed=waitExit(worker,2500);worker.kill("SIGKILL");await killed.catch(()=>{})}
  }
  process.exit(code);
}
for(const signal of ["SIGTERM","SIGINT"])process.on(signal,()=>{void shutdown(0)});
try {
  if(!process.env.DATABASE_URL)throw Error("Browser fixture requires DATABASE_URL");
  const build=spawn("cargo",["build","--locked","-p","liar-server","--example","auth_fixture"],{cwd:root,stdio:"inherit",windowsHide:true});
  const [code]=await once(build,"exit");
  if(code!==0)throw Error("Fixture build failed");
  await start();
  server.listen(3002,"127.0.0.1");
  server.on("error",error=>{process.stderr.write(`Fixture control failed: ${error.message}\n`);void shutdown(1)});
} catch(error) {process.stderr.write(`${error.message}\n`);await shutdown(1)}
