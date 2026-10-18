import{createRoot}from'react-dom/client'
import{createPluginHostClient}from'@wonderland/plugin-ui-sdk'
import{App}from'./App'
import './host.css'
const host=createPluginHostClient('my_wonderland')
document.documentElement.dataset.wonderlandRemote=String(new URLSearchParams(location.search).get('wonderlandClient')==='web')
createRoot(document.getElementById('root')!).render(<App host={host}/>)
