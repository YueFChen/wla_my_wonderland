import {useCallback,useEffect,useMemo,useRef,useState}from'react'
import type{PluginHostClient}from'@wonderland/plugin-ui-sdk'
import{MyWonderlandPage,type WonderlandApi}from'./index'
import{AccountManagement,type AccountApi}from'./accounts'
import type{AccountSnapshot,Account,Scope,Series,Platform}from'./types.generated'
import{t}from'./i18n'
import{WonderlandPanelContext}from'./Charts'
import { HistoryActions } from './HistoryActions'

export function App({host}:{host:PluginHostClient}){
 const[snapshot,setSnapshot]=useState<AccountSnapshot|null>(null),[error,setError]=useState(''),[panel,setPanel]=useState<string|null>(null),[selection,setSelection]=useState<string|null>(null)
 const live=useRef(true),generation=useRef(0)
 const [activeRevision,setActiveRevision]=useState(0)
 const remote=new URLSearchParams(location.search).get('wonderlandClient')==='web'
 const refresh=useCallback(async()=>{const request=++generation.current;try{const next=await host.call<AccountSnapshot>('account_snapshot');if(live.current&&request===generation.current){setSnapshot(next);setError('')}}catch(cause){if(live.current&&request===generation.current)setError(cause instanceof Error?cause.message:String(cause))}},[host])
 const api=useMemo<WonderlandApi>(()=>({accountSnapshot:()=>host.call<AccountSnapshot>('account_snapshot'),collect:(scope:Scope)=>host.call<Series>('collect',{scope}),series:(scope:Scope)=>host.call<Series|null>('series',{scope}),rememberScope:(key:string)=>host.preferences.set('selected-scope',key)}),[host])
 const accountApi=useMemo<AccountApi>(()=>({login:(platform:Platform)=>host.call<AccountSnapshot>('account_login',{platform}),cancel:()=>host.call('account_login_cancel'),sync:(account_key:string)=>host.call<Account>('account_sync',{account_key}),switch:(account_key:string)=>host.call('account_switch',{account_key}),remove:(account_key:string)=>host.call('account_remove',{account_key}),refresh}),[host,refresh])
 useEffect(()=>{
  live.current=true;const stops:Array<()=>void>=[]
  const keep=(promise:Promise<()=>void>)=>void promise.then(stop=>{if(live.current)stops.push(stop);else stop()}).catch(()=>{})
  keep(host.followHostTheme(({resolved})=>{document.documentElement.dataset.theme=resolved}))
  const activate=()=>{void refresh();setActiveRevision(value=>value+1)}
  keep(host.onSurfaceLifecycle(state=>{document.documentElement.dataset.surfaceState=state;if(state==='active')activate()}))
  keep(host.subscribe('account.changed',()=>void refresh()))
  void refresh()
  void host.preferences.get('selected-scope').then(value=>{if(live.current)setSelection(typeof value==='string'?value:'')}).catch(()=>{if(live.current)setSelection('')})
  const focus=activate;const visible=()=>{if(document.visibilityState==='visible')activate()}
  window.addEventListener('focus',focus);document.addEventListener('visibilitychange',visible)
  return()=>{live.current=false;generation.current++;stops.forEach(stop=>stop());window.removeEventListener('focus',focus);document.removeEventListener('visibilitychange',visible)}
 },[host,refresh])
 const panels=useMemo(()=>({active:panel,select:setPanel}),[panel])
 return <WonderlandPanelContext.Provider value={panels}>
  <HistoryActions host={host} onImported={()=>setActiveRevision(value=>value+1)}/>
  {error&&<div className="mw mw-account-error" role="alert">{error}<button className="app-button" onClick={()=>void refresh()}>{t('account.retry')}</button></div>}
  {selection!==null&&<MyWonderlandPage api={api} snapshot={snapshot} initialSelection={selection} activeRevision={activeRevision} remote={remote} onManageAccounts={remote?undefined:()=>setPanel('accounts')}/>}
  {!remote&&<AccountManagement open={panel==='accounts'} onClose={()=>setPanel(null)} snapshot={snapshot} api={accountApi}/>}
 </WonderlandPanelContext.Provider>
}
