// Real backend, synthetic browser/HTTP, temporary data; no real login or credentials.
import assert from 'node:assert/strict'
import {spawn}from'node:child_process'
import{mkdtemp,mkdir,readFile,writeFile,cp}from'node:fs/promises'
import{tmpdir}from'node:os'
import path from'node:path'
import{createInterface}from'node:readline'
import{fileURLToPath}from'node:url'
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..'),binary=path.resolve(process.argv[2]??path.join(root,'target/debug/wonderland-my-wonderland.exe'))
const base=await mkdtemp(path.join(tmpdir(),'my-wonderland-account-')),data=path.join(base,'plugin'),legacy=path.join(base,'legacy')
await mkdir(path.join(legacy,'account/credentials'),{recursive:true});await mkdir(path.join(data,'archive-v1'),{recursive:true})
const original=Buffer.from('{"accounts":[],"current_account":""}'),sealed=Buffer.from('synthetic old orphan encrypted bytes')
await writeFile(path.join(legacy,'account/accounts.json'),original);await writeFile(path.join(legacy,'account/credentials/orphan.bin'),sealed);await writeFile(path.join(data,'archive-v1/keep'),Buffer.from('retained'))
const scope={account_key:'demo',uid:'100001',region:'cn_gf01'},pause=ms=>new Promise(resolve=>setTimeout(resolve,ms))
class Harness{
 constructor(){
  this.pending=new Map();this.streams=new Map();this.sessions=new Set();this.authenticated=[];this.serial=0;this.issueCookies=false;this.stderr=''
  this.child=spawn(binary,[],{windowsHide:true,env:{...process.env,WONDERLAND_PLUGIN_DATA_DIR:data,WONDERLAND_HOST_DATA_DIR:legacy},stdio:['pipe','pipe','pipe']})
  this.child.stderr.on('data',bytes=>{this.stderr+=bytes});this.child.on('error',error=>{this.failure=error});this.lines=createInterface({input:this.child.stdout})
  this.lines.on('line',line=>{try{const frame=JSON.parse(line);if(frame.type==='hello'){this.hello=frame;return}if(frame.type==='event')return;if(frame.type==='request'){void this.core(frame).then(result=>this.send({type:'result',id:frame.id,result}),error=>{this.failure=error;this.send({type:'error',id:frame.id,error:{code:'FIXTURE_FAILURE',message:error.message}})});return}const call=this.pending.get(frame.id);assert(call,'Unknown backend response');this.pending.delete(frame.id);clearTimeout(call.timer);frame.type==='error'?call.reject(Object.assign(Error(frame.error.message),{code:frame.error.code})):call.resolve(frame.result)}catch(error){this.failure=error}})
  this.send({type:'hello',role:'host',pluginId:'my_wonderland'})
 }
 send(frame){this.child.stdin.write(JSON.stringify({protocol:'wonderland-plugin',version:'1.0.0',...frame})+'\n')}
 async until(predicate){const deadline=Date.now()+15000;while(!predicate()){if(this.failure)throw this.failure;assert.equal(this.child.exitCode,null,this.stderr);assert(Date.now()<deadline,'Fixture timeout');await pause(10)}}
 request(method,params={},serviceContext='device'){const id='fixture-'+(++this.serial);const promise=new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(Error('RPC timeout: '+method)),15000);this.pending.set(id,{resolve,reject,timer});this.send({type:'request',id,method,params,serviceContext})});return{id,promise}}
 rpc(method,params={},context='device'){return this.request(method,params,context).promise}
 async core(frame){
  assert(!frame.method.startsWith('core.account.'),'The plugin must own its account business')
  const p=frame.params??{}
  switch(frame.method){
   case'core.browser.session.open':{assert.equal(frame.serviceContext,this.loginContext);assert.equal(p.profile,'mihoyo-login');assert.equal(p.reset,true);this.sessions.add('browser');return{sessionId:'browser'}}
   case'core.browser.session.cookies':assert(this.sessions.has(p.sessionId));return this.issueCookies?[{name:'ltuid',value:'demo',domain:'.mihoyo.com',path:'/'},{name:'ltoken',value:'synthetic-cookie-token',domain:'.mihoyo.com',path:'/'}]:[]
   case'core.browser.session.navigate':case'core.browser.session.visibility':return{ok:true}
   case'core.browser.session.close':this.sessions.delete(p.sessionId);return{ok:true}
   case'core.network.request':{
    const url=new URL(p.url);assert(['api-takumi.mihoyo.com','api-micreator.mihoyo.com'].includes(url.hostname));const cookie=p.headers.find(h=>h.name.toLowerCase()==='cookie');assert(Buffer.from(cookie.value).toString().includes('synthetic-cookie-token'))
    this.authenticated.push({path:url.pathname,context:frame.serviceContext});let body
    if(url.pathname.includes('getUserGameRolesByCookie'))body={retcode:0,data:{list:[{game_biz:'hk4e_cn',game_uid:'100001',region:'cn_gf01',region_name:'Fixture',nickname:'Fixture',level:60}]}}
    else if(url.pathname.includes('/creator'))body={retcode:0,data:{kolugc_level:3,cur_level:3,cur_exp:12,total_exp:100}}
    else{assert.equal(frame.serviceContext,'collect-device','Another request changed the collection context');if(this.hold&&url.pathname.endsWith('get_user_info'))await this.hold;body=JSON.parse(await readFile(path.join(root,'tests/fixtures',url.pathname.endsWith('get_user_info')?'overview.json':'empty.json'),'utf8'))}
    const id='body-'+this.serial+'-'+this.authenticated.length,bytes=Buffer.from(JSON.stringify(body));this.streams.set(id,{bytes,context:frame.serviceContext});return{status:200,headers:[],finalUrl:p.url,body:{streamId:id,size:bytes.length}}
   }
   case'core.streams.read':{const stream=this.streams.get(p.streamId);assert(stream);assert.equal(frame.serviceContext,stream.context);this.streams.delete(p.streamId);return{contentBase64:stream.bytes.toString('base64'),eof:true}}
   case'core.streams.close':this.streams.delete(p.streamId);return{ok:true}
   default:throw Error('Unexpected host service '+frame.method)
  }
 }
 async stop(){if(this.stopped)return;this.stopped=true;for(const item of this.pending.values())clearTimeout(item.timer);this.child.stdin.end();await Promise.race([new Promise(resolve=>this.child.once('exit',resolve)),pause(700)]);if(this.child.exitCode===null)this.child.kill();this.lines.close()}
}
let first=new Harness(),second,third
try{
 await first.until(()=>first.hello);assert.equal((await first.rpc('account_snapshot')).accounts.length,0)
 assert.deepEqual(await readFile(path.join(data,'account/accounts.json')),original);assert.deepEqual(await readFile(path.join(data,'account/credentials/orphan.bin')),sealed)
 first.issueCookies=true;first.loginContext='login-device';const logged=await first.rpc('account_login',{},'login-device');assert.equal(logged.current_account_key,'demo');assert.equal(logged.accounts[0].game_roles[0].uid,'100001');assert.equal(first.sessions.size,0)
 assert(!(await readFile(path.join(data,'account/credentials/demo.bin'))).includes(Buffer.from('synthetic-cookie-token')))
 await cp(path.join(data,'account'),path.join(base,'accepted-login/account'),{recursive:true})
 await first.stop();second=new Harness();await second.until(()=>second.hello);assert.equal((await second.rpc('account_snapshot')).current_account_key,'demo')
 let release;second.hold=new Promise(resolve=>{release=resolve});const collecting=second.rpc('collect',{scope},'collect-device');await second.until(()=>second.authenticated.length>0)
 assert.equal((await second.rpc('account_snapshot',{},'another-device')).accounts[0].account_key,'demo');release();const series=await collecting;assert.equal(series.fetch_count,1);assert.equal(second.authenticated.length,2)
 second.loginContext='cancel-device';const pending=second.request('account_login',{},'cancel-device');await second.until(()=>second.sessions.size===1);second.send({type:'cancel',id:'unrelated'});assert.equal((await second.rpc('account_snapshot')).status,'logging_in');second.send({type:'cancel',id:pending.id});assert.equal((await pending.promise).last_login_failure,'cancelled');assert.equal(second.sessions.size,0)
 await second.rpc('account_remove',{account_key:'demo'});assert.equal((await second.rpc('account_snapshot')).accounts.length,0);assert.equal((await second.rpc('series',{scope})).fetch_count,1)
 await second.stop();third=new Harness();await third.until(()=>third.hello);assert.equal((await third.rpc('account_snapshot')).accounts.length,0);assert.equal((await third.rpc('series',{scope})).fetch_count,1)
 assert.deepEqual(await readFile(path.join(legacy,'account/accounts.json')),original);assert.deepEqual(await readFile(path.join(legacy,'account/credentials/orphan.bin')),sealed);assert.equal(await readFile(path.join(data,'archive-v1/keep'),'utf8'),'retained')
 if(first.failure||second.failure||third.failure)throw first.failure??second.failure??third.failure
 console.log('PASS: plugin-owned login, encrypted restart, original bytes, collection context, cancellation and offline records. Fixture: '+base)
}finally{await first.stop();if(second)await second.stop();if(third)await third.stop()}
