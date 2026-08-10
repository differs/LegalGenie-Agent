import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '../index.css'
import { OverviewPage } from '../site/pages'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <OverviewPage />
  </StrictMode>,
)
