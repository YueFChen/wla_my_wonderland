import{useEffect,useRef,useState}from'react'
import{Drawer,DialogButton,confirmDialog}from'@wonderland/ui'
import type{Account,AccountSnapshot}from'./types.generated'
import{t}from'./i18n'
import './accounts.css'
export interface AccountApi{login():Promise<AccountSnapshot>;cancel():Promise<unknown>;sync(key:string):Promise<Account>;switch(key:string):Promise<unknown>;remove(key:string):Promise<unknown>;refresh():Promise<void>}
export function AccountManagement({open,onClose,snapshot,api}:{open:boolean;onClose:()=>void;snapshot:AccountSnapshot|null;api:AccountApi}){
 const[busy,setBusy]=useState(false),[error,setError]=useState('');const mounted=useRef(true)
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false}},[])
 const run=async(action:()=>Promise<unknown>)=>{setBusy(true);setError('');try{await action()}catch(cause){if(mounted.current)setError(cause instanceof Error?cause.message:String(cause))}finally{await api.refresh();if(mounted.current)setBusy(false)}}
 const logging=snapshot?.status==='logging_in'
 const forget=async(account:Account)=>{if(await confirmDialog({title:t('account.forgetTitle'),message:t('account.forgetMessage'),confirmLabel:t('account.forget'),danger:true}))await run(()=>api.remove(account.account_key))}
 const failures={window_closed:t('account.windowClosed'),timeout:t('account.timeout'),cancelled:t('account.cancelled'),failed:t('account.failed')}
 return <Drawer open={open} dismissible={!busy} title={t('account.title')} onClose={onClose} closeLabel={t('account.close')} size="lg">
  <div className="mw-accounts">
   <div className="mw-accounts-actions">{logging?<><span role="status">{t('account.logging')}</span><DialogButton onClick={()=>void api.cancel().then(()=>api.refresh()).catch(cause=>setError(String(cause)))}>{t('account.cancelLogin')}</DialogButton></>:<DialogButton variant="primary" disabled={busy} onClick={()=>void run(api.login)}>{t('account.login')}</DialogButton>}</div>
   {error&&<p className="mw-account-error" role="alert">{error}</p>}
   {!logging&&snapshot?.last_login_failure&&!error&&<p className="mw-accounts-status" role="status">{failures[snapshot.last_login_failure]}</p>}
   {(snapshot?.accounts??[]).map(account=><article className="mw-account" key={account.account_key}>
    <div className="mw-account-head"><div className="mw-account-avatar">{account.avatar_url?<img src={account.avatar_url} alt=""/>:<span>{(account.game_roles[0]?.nickname??account.account_key).slice(0,1)}</span>}</div><div className="mw-account-copy"><strong>{account.game_roles[0]?.nickname??account.account_key}</strong><span>{t('account.passport')} {account.account_key}{account.creator_level?` · ${t('account.creator')} Lv.${account.creator_level}`:''}</span></div>{snapshot?.current_account_key===account.account_key&&<span className="mw-account-default">{t('account.default')}</span>}</div>
    {account.game_roles.length>0&&<ul className="mw-account-roles">{account.game_roles.map(role=><li key={`${role.region}/${role.uid}`}><span>{role.nickname}</span><span>{role.region_name} · {role.uid} · Lv.{role.level}</span></li>)}</ul>}
    <div className="mw-account-buttons"><DialogButton disabled={busy||logging} onClick={()=>void run(()=>api.sync(account.account_key))}>{t('account.sync')}</DialogButton>{snapshot?.current_account_key!==account.account_key&&<DialogButton disabled={busy||logging} onClick={()=>void run(()=>api.switch(account.account_key))}>{t('account.setDefault')}</DialogButton>}<DialogButton variant="danger" disabled={busy||logging} onClick={()=>void forget(account)}>{t('account.forget')}</DialogButton></div>
   </article>)}
   {snapshot?.accounts.length===0&&!logging&&<p className="mw-accounts-status">{t('account.empty')}</p>}
  </div>
 </Drawer>
}
